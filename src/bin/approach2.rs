use std::time::Instant;
use std::f64::consts::PI;
use geo::{Distance, Haversine};
use geo::Point as GeoPoint;
use tfhe::prelude::*;
use tfhe::{generate_keys, set_server_key, ConfigBuilder, FheUint32, ClientKey, FheBool};

// This binary implements the "Summary of the (enhanced) proposed solution" flow.
// Optimization: omit Step 4 (arcsin/sqrt) and Step 5 (multiply by Earth's radius).
// We compare distances by comparing the 'a' term directly.

pub const SCALE_FACTOR: u32 = 1_000_000;

#[derive(Debug)]
pub struct Point {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

pub struct ClientData {
    pub name: Option<String>,
    pub lat_rad: FheUint32,
    pub lon_rad: FheUint32,
    pub sin_lat: FheUint32,
    pub cos_lat: FheUint32,
}

fn precompute_client_data(
    lat_degrees: f64,
    lon_degrees: f64,
    name: Option<String>,
    client_key: &ClientKey,
) -> Result<(ClientData, u128), Box<dyn std::error::Error>> {
    let start = Instant::now();

    let lat_radians = lat_degrees * PI / 180.0;
    let lon_radians = lon_degrees * PI / 180.0;

    let sin_lat_val = lat_radians.sin();
    let cos_lat_val = lat_radians.cos();

    let scaled_lat_rad = (lat_radians * SCALE_FACTOR as f64) as u32;
    let scaled_lon_rad = (lon_radians * SCALE_FACTOR as f64) as u32;
    let scaled_sin_lat = ((sin_lat_val + 1.0) * SCALE_FACTOR as f64 / 2.0) as u32;
    let scaled_cos_lat = ((cos_lat_val + 1.0) * SCALE_FACTOR as f64 / 2.0) as u32;

    let encrypted_lat_rad = FheUint32::try_encrypt(scaled_lat_rad, client_key)?;
    let encrypted_lon_rad = FheUint32::try_encrypt(scaled_lon_rad, client_key)?;
    let encrypted_sin_lat = FheUint32::try_encrypt(scaled_sin_lat, client_key)?;
    let encrypted_cos_lat = FheUint32::try_encrypt(scaled_cos_lat, client_key)?;

    Ok((ClientData {
        name,
        lat_rad: encrypted_lat_rad,
        lon_rad: encrypted_lon_rad,
        sin_lat: encrypted_sin_lat,
        cos_lat: encrypted_cos_lat,
    }, start.elapsed().as_micros()))
}

fn compute_a_term(
    p1: &ClientData,
    p2: &ClientData,
) -> (FheUint32, Vec<(String, u128)>) {
    let mut timings: Vec<(String, u128)> = Vec::new();

    // SERVER: delta computations (scaled radians)
    let t0 = Instant::now();
    let delta_lat = &p1.lat_rad - &p2.lat_rad;
    let delta_lon_raw = &p1.lon_rad - &p2.lon_rad;
    let delta_lon_alt = &p2.lon_rad - &p1.lon_rad;
    let delta_lon = delta_lon_raw.min(&delta_lon_alt);
    timings.push(("server:step2:compute_deltas".to_string(), t0.elapsed().as_micros()));

    // SERVER: sin^2(x/2) polynomial approximation for dlat and dlon
    let t1 = Instant::now();
    let lat2 = &delta_lat * &delta_lat;
    let lat4 = &lat2 * &lat2;
    let lat6 = &lat4 * &lat2;
    let lat8 = &lat6 * &lat2;
    let lat10 = &lat8 * &lat2;

    let sin2_half_dlat = &lat2 / 4_u32
        - &lat4 / 192_u32
        + &lat6 / 23040_u32
        - &lat8 / 5160960_u32
        + &lat10 / 1486356480_u32;

    let lon2 = &delta_lon * &delta_lon;
    let lon4 = &lon2 * &lon2;
    let lon6 = &lon4 * &lon2;
    let lon8 = &lon6 * &lon2;
    let lon10 = &lon8 * &lon2;

    let sin2_half_dlon = &lon2 / 4_u32
        - &lon4 / 192_u32
        + &lon6 / 23040_u32
        - &lon8 / 5160960_u32
        + &lon10 / 1486356480_u32;
    timings.push(("server:step3:poly_sin2_half".to_string(), t1.elapsed().as_micros()));

    // SERVER: a = sin^2(dlat/2) + cos(lat1)cos(lat2)sin^2(dlon/2)
    let t2 = Instant::now();
    let cos_prod = &p1.cos_lat * &p2.cos_lat / SCALE_FACTOR;
    let a = &sin2_half_dlat + &cos_prod * &sin2_half_dlon;
    timings.push(("server:step3:combine_a".to_string(), t2.elapsed().as_micros()));

    (a, timings)
}

fn compare_distances(
    px: &ClientData,
    py: &ClientData,
    pz: &ClientData,
) -> (FheBool, Vec<(String, u128)>) {
    let mut timings: Vec<(String, u128)> = Vec::new();

    let t_x = Instant::now();
    let (a_xz, mut t_xz) = compute_a_term(px, pz);
    let name_x = px.name.as_deref().unwrap_or("X");
    let name_z = pz.name.as_deref().unwrap_or("Z");
    timings.push((format!("server:step3:compute_a_{}-{}", name_x, name_z), t_x.elapsed().as_micros()));
    timings.append(&mut t_xz);

    let t_y = Instant::now();
    let (a_yz, mut t_yz) = compute_a_term(py, pz);
    let name_y = py.name.as_deref().unwrap_or("Y");
    timings.push((format!("server:step3:compute_a_{}-{}", name_y, name_z), t_y.elapsed().as_micros()));
    timings.append(&mut t_yz);

    let t_cmp = Instant::now();
    let res = a_xz.lt(&a_yz);
    timings.push(("server:final:compare".to_string(), t_cmp.elapsed().as_micros()));

    (res, timings)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let default_points = vec![
        Point { name: "Basel".to_string(), lat: 47.5596, lon: 7.5886 },
        Point { name: "Lugano".to_string(), lat: 46.0037, lon: 8.9511 },
        Point { name: "Zurich".to_string(), lat: 47.3769, lon: 8.5417 },
    ];
    let args: Vec<String> = std::env::args().collect();
    let points = if args.len() == 10 {
        vec![
            Point { name: args[1].clone(), lat: args[2].parse()?, lon: args[3].parse()? },
            Point { name: args[4].clone(), lat: args[5].parse()?, lon: args[6].parse()? },
            Point { name: args[7].clone(), lat: args[8].parse()?, lon: args[9].parse()? },
        ]
    } else {
        default_points
    };

    // CLIENT: keygen (excluded from timings, but done per run)
    let keygen_start = Instant::now();
    let config = ConfigBuilder::default().build();
    let (client_key, server_keys) = generate_keys(config);
    set_server_key(server_keys);
    let keygen_us = keygen_start.elapsed().as_micros();

    // CLIENT: precompute + encrypt per point
    let mut client_timings = Vec::new();
    let (x, t_x) = precompute_client_data(points[0].lat, points[0].lon, Some(points[0].name.clone()), &client_key)?;
    client_timings.push((format!("client:step1:precompute+encrypt:{}", points[0].name), t_x));
    let (y, t_y) = precompute_client_data(points[1].lat, points[1].lon, Some(points[1].name.clone()), &client_key)?;
    client_timings.push((format!("client:step1:precompute+encrypt:{}", points[1].name), t_y));
    let (z, t_z) = precompute_client_data(points[2].lat, points[2].lon, Some(points[2].name.clone()), &client_key)?;
    client_timings.push((format!("client:step1:precompute+encrypt:{}", points[2].name), t_z));

    // SERVER: compute and compare using 'a' directly
    let server_start = Instant::now();
    let (is_x_closer_ct, mut server_timings) = compare_distances(&x, &y, &z);
    let server_total_us = server_start.elapsed().as_micros();

    // CLIENT: decrypt comparison bit
    let t_dec = Instant::now();
    let is_x_closer = is_x_closer_ct.decrypt(&client_key);
    let client_decrypt_us = t_dec.elapsed().as_micros();

    // Non-FHE baseline using geo::Haversine
    let baseline_start = Instant::now();
    let gx = GeoPoint::new(points[0].lon, points[0].lat);
    let gy = GeoPoint::new(points[1].lon, points[1].lat);
    let gz = GeoPoint::new(points[2].lon, points[2].lat);
    let xz_km = Haversine.distance(gx, gz) / 1000.0;
    let yz_km = Haversine.distance(gy, gz) / 1000.0;
    let baseline_us = baseline_start.elapsed().as_micros();

    println!("CLIENT: key generation (excluded) = {:.6} s", (keygen_us as f64) / 1_000_000.0);
    for (label, us) in client_timings.iter() { println!("{} = {:.6} s", label, (*us as f64) / 1_000_000.0); }
    println!("SERVER: total compute = {:.6} s", (server_total_us as f64) / 1_000_000.0);
    for (label, us) in server_timings.iter() { println!("{} = {:.6} s", label, (*us as f64) / 1_000_000.0); }
    println!("CLIENT: decrypt compare bit = {:.6} s", (client_decrypt_us as f64) / 1_000_000.0);

    let client_total_us: u128 = client_timings.iter().map(|(_, us)| *us).sum::<u128>() + client_decrypt_us;
    println!("CLIENT: TOTAL = {:.6} s", (client_total_us as f64) / 1_000_000.0);
    println!("SERVER: TOTAL = {:.6} s", (server_total_us as f64) / 1_000_000.0);

    println!("\nResult (FHE): X is {} to Z than Y", if is_x_closer { "closer" } else { "further" });
    println!("Baseline (geo): XZ = {:.3} km, YZ = {:.3} km ({} µs)", xz_km, yz_km, baseline_us);

    Ok(())
}



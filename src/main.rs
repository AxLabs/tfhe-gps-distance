use tfhe::prelude::*;
use tfhe::{generate_keys, set_server_key, ConfigBuilder, FheUint32, ClientKey, FheBool};
use std::time::Instant;
use std::f64::consts::PI;

// Scale factors for fixed-point arithmetic
const SCALE_FACTOR: u32 = 10_000;
const EARTH_RADIUS_KM: u32 = 6371;

// Client-side precomputed values
struct ClientData {
    name: Option<String>,   // Optional name for the point
    lat: FheUint32,         // Encrypted latitude in degrees (scaled)
    lon: FheUint32,         // Encrypted longitude in degrees (scaled)
    cos_lat: FheUint32,     // Encrypted cosine of latitude
    sin_lat: FheUint32,     // Encrypted sine of latitude
}

// Function to precompute and encrypt client data (GPS coordinates & trig values)
fn precompute_client_data(
    lat_degrees: f64, 
    lon_degrees: f64,
    name: Option<String>,
    client_key: &ClientKey
) -> Result<ClientData, Box<dyn std::error::Error>> {
    let point_desc = name.as_deref().map_or("", |n| n);
    let formatted_desc = if point_desc.is_empty() { 
        String::new() 
    } else { 
        format!(" ({})", point_desc)
    };
    println!("Precomputing values for coordinate{}: {:.4}° N, {:.4}° E", 
             formatted_desc, lat_degrees, lon_degrees);
    
    // Convert to radians
    let lat_radians = lat_degrees * PI / 180.0;
    
    // Calculate sine and cosine
    let sin_lat_val = lat_radians.sin();
    let cos_lat_val = lat_radians.cos();
    
    // Scale values
    let scaled_lat = (lat_degrees * SCALE_FACTOR as f64) as u32;
    let scaled_lon = (lon_degrees * SCALE_FACTOR as f64) as u32;
    
    // Scale trig values from [-1,1] to [0,SCALE_FACTOR]
    let scaled_sin_lat = ((sin_lat_val + 1.0) * SCALE_FACTOR as f64 / 2.0) as u32;
    let scaled_cos_lat = ((cos_lat_val + 1.0) * SCALE_FACTOR as f64 / 2.0) as u32;
    
    println!("  Original cos(lat): {:.4}, Scaled cos(lat): {}", cos_lat_val, scaled_cos_lat);
    
    // Encrypt all values
    let encrypted_lat = FheUint32::try_encrypt(scaled_lat, client_key)?;
    let encrypted_lon = FheUint32::try_encrypt(scaled_lon, client_key)?;
    let encrypted_sin_lat = FheUint32::try_encrypt(scaled_sin_lat, client_key)?;
    let encrypted_cos_lat = FheUint32::try_encrypt(scaled_cos_lat, client_key)?;
    
    Ok(ClientData {
        name,
        lat: encrypted_lat,
        lon: encrypted_lon,
        sin_lat: encrypted_sin_lat,
        cos_lat: encrypted_cos_lat,
    })
}

// Calculate approximate squared distance between points using Haversine formula
// For small angles, sin²(θ/2) ≈ (θ/2)², so we can approximate:
// hav(d/r) = hav(Δϕ) + cos(ϕ₁)·cos(ϕ₂)·hav(Δλ)
// where hav(θ) = sin²(θ/2)
fn calculate_haversine_distance_squared(
    point1: &ClientData,
    point2: &ClientData,
) -> FheUint32 {
    // Calculate delta latitude and longitude
    // We need to handle the case where point2.lat > point1.lat and vice versa
    // Since we're working with unsigned integers, we need to determine the order
    let delta_lat_1 = &point1.lat - &point2.lat;
    let delta_lat_2 = &point2.lat - &point1.lat;
    
    // Similarly for longitude
    let delta_lon_1 = &point1.lon - &point2.lon;
    let delta_lon_2 = &point2.lon - &point1.lon;
    
    // Use the smaller of the two deltas for both lat and lon
    let delta_lat = delta_lat_1.min(&delta_lat_2);
    let delta_lon = delta_lon_1.min(&delta_lon_2);
    
    // Calculate hav(Δϕ) ≈ (Δϕ/2)²
    // We divide by 2 first to avoid overflow
    let delta_lat_half = &delta_lat / 2_u32;
    let hav_delta_lat = &delta_lat_half * &delta_lat_half;
    
    // Get the cosine values and rescale from [0,SCALE_FACTOR] back to [0,1]
    // First, shift back to [-SCALE_FACTOR/2, SCALE_FACTOR/2]
    let rescaled_cos_lat1 = &point1.cos_lat * 2_u32 - SCALE_FACTOR;
    let rescaled_cos_lat2 = &point2.cos_lat * 2_u32 - SCALE_FACTOR;
    
    // Calculate hav(Δλ) ≈ (Δλ/2)²
    let delta_lon_half = &delta_lon / 2_u32;
    let hav_delta_lon = &delta_lon_half * &delta_lon_half;
    
    // Calculate cos(ϕ₁)·cos(ϕ₂)·hav(Δλ)
    // We multiply by cos_lat and then divide by SCALE_FACTOR to get the proper scaling
    let weighted_hav_delta_lon = &hav_delta_lon * (rescaled_cos_lat1 + rescaled_cos_lat2) / (2_u32 * SCALE_FACTOR);
    
    // Sum the terms to get the approximate haversine distance squared
    let distance_squared = &hav_delta_lat + &weighted_hav_delta_lon;
    
    distance_squared
}

// Compare which point is closer to the reference point
fn compare_distances(
    point_x: &ClientData,
    point_y: &ClientData,
    reference_z: &ClientData,
) -> FheBool {
    println!("Calculating approximate distance from X to Z...");
    let dist_x_to_z_squared = calculate_haversine_distance_squared(point_x, reference_z);
    
    println!("Calculating approximate distance from Y to Z...");
    let dist_y_to_z_squared = calculate_haversine_distance_squared(point_y, reference_z);
    
    println!("Comparing distances...");
    // Return true if X is closer to Z than Y is to Z
    dist_x_to_z_squared.lt(&dist_y_to_z_squared)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting GPS distance comparison using FHE...");
    println!("Following Haversine formula approximation with client precomputation");

    // Configure TFHE
    let config = ConfigBuilder::default().build();
    let (client_key, server_keys) = generate_keys(config);
    set_server_key(server_keys);

    // Client-side: Precompute and encrypt data for 3 points
    println!("\n1. CLIENT SIDE: Precomputing and encrypting coordinates");
    
    // Point X
    let x_lat = 47.5596;
    let x_lon = 7.5886;
    let x_name = Some("Basel".to_string());
    let x_desc = if let Some(n) = x_name.as_deref() { format!(" ({})", n) } else { String::new() };
    println!("Point X{}: Latitude {:.4}° N, Longitude {:.4}° E", x_desc, x_lat, x_lon);
    let client_data_x = precompute_client_data(x_lat, x_lon, x_name.clone(), &client_key)?;
    
    // Point Y
    let y_lat = 46.0037;
    let y_lon = 8.9511;
    let y_name = Some("Lugano".to_string());
    let y_desc = if let Some(n) = y_name.as_deref() { format!(" ({})", n) } else { String::new() };
    println!("Point Y{}: Latitude {:.4}° N, Longitude {:.4}° E", y_desc, y_lat, y_lon);
    let client_data_y = precompute_client_data(y_lat, y_lon, y_name.clone(), &client_key)?;
    
    // Point Z (reference point)
    let z_lat = 47.3769;
    let z_lon = 8.5417;
    let z_name = Some("Zurich".to_string());
    let z_desc = if let Some(n) = z_name.as_deref() { format!(" ({})", n) } else { String::new() };
    println!("Point Z{}: Latitude {:.4}° N, Longitude {:.4}° E", z_desc, z_lat, z_lon);
    let client_data_z = precompute_client_data(z_lat, z_lon, z_name.clone(), &client_key)?;

    // For debugging: verify the actual scaling
    println!("\nPlaintext calculations for verification:");
    println!("Distance X-Z: {:.2} km", haversine_distance(x_lat, x_lon, z_lat, z_lon));
    println!("Distance Y-Z: {:.2} km", haversine_distance(y_lat, y_lon, z_lat, z_lon));
    
    let approx_dist_xz = approximate_haversine_distance(x_lat, x_lon, z_lat, z_lon);
    let approx_dist_yz = approximate_haversine_distance(y_lat, y_lon, z_lat, z_lon);
    println!("Approximate distance X-Z: {:.2} units", approx_dist_xz);
    println!("Approximate distance Y-Z: {:.2} units", approx_dist_yz);
    println!("X should be closer: {}", approx_dist_xz < approx_dist_yz);

    // Server-side: Calculate and compare distances
    println!("\n2. SERVER SIDE: Performing FHE computations on encrypted data");
    let start_time = Instant::now();
    
    let closer_x = compare_distances(&client_data_x, &client_data_y, &client_data_z);
    
    let duration = start_time.elapsed();

    // Client-side: Decrypt final result
    println!("\n3. CLIENT SIDE: Decrypting the result");
    let is_x_closer: bool = closer_x.decrypt(&client_key);

    // Display results
    println!("\nResults:");
    if is_x_closer {
        println!("Point X{} is closer to point Z{}.", x_desc, z_desc);
    } else {
        println!("Point Y{} is closer to point Z{}.", y_desc, z_desc);
    }
    
    // Calculate actual distances for verification
    let actual_dist_xz = haversine_distance(x_lat, x_lon, z_lat, z_lon);
    let actual_dist_yz = haversine_distance(y_lat, y_lon, z_lat, z_lon);
    
    println!("\nVerification with plaintext calculation:");
    println!("Actual distance X-Z: {:.2} km", actual_dist_xz);
    println!("Actual distance Y-Z: {:.2} km", actual_dist_yz);
    println!("Computation time for FHE comparison: {:?}", duration);

    Ok(())
}

// Function to calculate the approximate Haversine distance for verification
fn approximate_haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    // Convert to radians
    let lat1_rad = lat1 * PI / 180.0;
    let lat2_rad = lat2 * PI / 180.0;
    
    // Calculate delta in degrees
    let delta_lat = (lat2 - lat1).abs();
    let delta_lon = (lon2 - lon1).abs();
    
    // Calculate the average cosine
    let avg_cos = (lat1_rad.cos() + lat2_rad.cos()) / 2.0;
    
    // Calculate squared distance using small-angle approximation
    // hav(d/r) = hav(Δϕ) + cos(ϕ₁)·cos(ϕ₂)·hav(Δλ)
    // where hav(θ) ≈ (θ/2)² for small angles
    let hav_delta_lat = (delta_lat / 2.0).powi(2);
    let hav_delta_lon = (delta_lon / 2.0).powi(2);
    
    let dist_squared = hav_delta_lat + avg_cos * hav_delta_lon;
    
    dist_squared.sqrt()
}

// Helper function to calculate actual distance using Haversine formula (for verification)
fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    // Convert coordinates from degrees to radians
    let lat1_rad = lat1 * PI / 180.0;
    let lon1_rad = lon1 * PI / 180.0;
    let lat2_rad = lat2 * PI / 180.0;
    let lon2_rad = lon2 * PI / 180.0;
    
    // Calculate differences
    let delta_lat = lat2_rad - lat1_rad;
    let delta_lon = lon2_rad - lon1_rad;
    
    // Haversine formula
    let a = (delta_lat/2.0).sin().powi(2) + 
            lat1_rad.cos() * lat2_rad.cos() * (delta_lon/2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    
    // Distance in kilometers
    EARTH_RADIUS_KM as f64 * c
}

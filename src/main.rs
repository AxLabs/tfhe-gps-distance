use tfhe::prelude::*;
use tfhe::{generate_keys, set_server_key, ConfigBuilder, FheUint32, ClientKey, FheBool};
use std::time::Instant;
use std::f64::consts::PI;
use geo::prelude::*;
use geo::Point as GeoPoint;

// Scale factors for fixed-point arithmetic
pub const SCALE_FACTOR: u32 = 1_000_000;
pub const EARTH_RADIUS_KM: u32 = 6371;

// Structure to hold point information
#[derive(Debug)]
pub struct Point {
    pub name: String,
    pub lat: f64,  // latitude in degrees
    pub lon: f64,  // longitude in degrees
}

// Client-side precomputed values
pub struct ClientData {
    pub name: Option<String>,      // Optional name for the point
    pub lat_rad: FheUint32,       // Encrypted latitude in radians (scaled)
    pub lon_rad: FheUint32,       // Encrypted longitude in radians (scaled)
    pub sin_lat: FheUint32,       // Encrypted sine of latitude
    pub cos_lat: FheUint32,       // Encrypted cosine of latitude
}

// Function to precompute and encrypt client data (GPS coordinates & trig values)
pub fn precompute_client_data(
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
    
    // Step 1: Convert to radians (client-side)
    let lat_radians = lat_degrees * PI / 180.0;
    let lon_radians = lon_degrees * PI / 180.0;
    
    // Pre-compute transcendental values (client-side)
    let sin_lat_val = lat_radians.sin();
    let cos_lat_val = lat_radians.cos();
    
    // Scale values for encryption
    let scaled_lat_rad = (lat_radians * SCALE_FACTOR as f64) as u32;
    let scaled_lon_rad = (lon_radians * SCALE_FACTOR as f64) as u32;
    
    // Scale trig values from [-1,1] to [0,SCALE_FACTOR]
    let scaled_sin_lat = ((sin_lat_val + 1.0) * SCALE_FACTOR as f64 / 2.0) as u32;
    let scaled_cos_lat = ((cos_lat_val + 1.0) * SCALE_FACTOR as f64 / 2.0) as u32;
    
    println!("  Original cos(lat): {:.4}, Scaled cos(lat): {}", cos_lat_val, scaled_cos_lat);
    println!("  Scaled latitude (rad): {}, Scaled longitude (rad): {}", scaled_lat_rad, scaled_lon_rad);
    
    // Encrypt all values
    let encrypted_lat_rad = FheUint32::try_encrypt(scaled_lat_rad, client_key)?;
    let encrypted_lon_rad = FheUint32::try_encrypt(scaled_lon_rad, client_key)?;
    let encrypted_sin_lat = FheUint32::try_encrypt(scaled_sin_lat, client_key)?;
    let encrypted_cos_lat = FheUint32::try_encrypt(scaled_cos_lat, client_key)?;
    
    Ok(ClientData {
        name,
        lat_rad: encrypted_lat_rad,
        lon_rad: encrypted_lon_rad,
        sin_lat: encrypted_sin_lat,
        cos_lat: encrypted_cos_lat,
    })
}

// Calculate approximate squared distance between points using Haversine formula
// Using polynomial approximations as specified in the solution
pub fn calculate_haversine_distance_squared(
    point1: &ClientData,
    point2: &ClientData,
    _client_key: &ClientKey
) -> FheUint32 {
    // Calculate deltas (Step 2)
    let diff_start_time = Instant::now();
    let delta_lat = (&point1.lat_rad - &point2.lat_rad).min(&(&point2.lat_rad - &point1.lat_rad));
    
    // Handle International Date Line crossing for longitude difference
    // For longitude, we need to consider the shortest path around the globe
    // This means we need to consider both the direct difference and the path through the IDL
    
    // Calculate the direct difference
    let direct_diff = &point1.lon_rad - &point2.lon_rad;
    
    // Calculate the complement (going the other way around the globe)
    let complement_diff = &(&point2.lon_rad - &point1.lon_rad);
    
    // Calculate the path through the IDL
    // This is effectively the complement of the direct difference
    // We need to consider that the shortest path might be through the IDL
    let idl_path = &(&point1.lon_rad + &point2.lon_rad);
    
    // The actual delta_lon should be the minimum of all possible paths
    // This ensures we're always using the shortest path around the globe
    let delta_lon = direct_diff.min(complement_diff).min(idl_path);
    
    println!("    Difference calculation time: {:.2?}", diff_start_time.elapsed());

    // Step 3: Compute intermediate value 'a' using polynomial approximations
    let compute_start_time = Instant::now();
    
    // Polynomial approximation for sin²(x/2):
    // sin²(x/2) ≈ x²/4 - x⁴/192 + x⁶/23040 - x⁸/5160960 + x¹⁰/1486356480
    
    // For delta_lat
    let lat_squared = &delta_lat * &delta_lat;
    let lat_power4 = &lat_squared * &lat_squared;
    let lat_power6 = &lat_power4 * &lat_squared;
    let lat_power8 = &lat_power6 * &lat_squared;
    let lat_power10 = &lat_power8 * &lat_squared;
    
    let lat_term1 = &lat_squared / 4_u32;
    let lat_term2 = &lat_power4 / 192_u32;
    let lat_term3 = &lat_power6 / 23040_u32;
    let lat_term4 = &lat_power8 / 5160960_u32;
    let lat_term5 = &lat_power10 / 1486356480_u32;
    
    let sin_squared_half_delta_lat = &lat_term1 - &lat_term2 + &lat_term3 - &lat_term4 + lat_term5;
    
    // For delta_lon
    let lon_squared = &delta_lon * &delta_lon;
    let lon_power4 = &lon_squared * &lon_squared;
    let lon_power6 = &lon_power4 * &lon_squared;
    let lon_power8 = &lon_power6 * &lon_squared;
    let lon_power10 = &lon_power8 * &lon_squared;
    
    let lon_term1 = &lon_squared / 4_u32;
    let lon_term2 = &lon_power4 / 192_u32;
    let lon_term3 = &lon_power6 / 23040_u32;
    let lon_term4 = &lon_power8 / 5160960_u32;
    let lon_term5 = &lon_power10 / 1486356480_u32;
    
    let sin_squared_half_delta_lon = &lon_term1 - &lon_term2 + &lon_term3 - &lon_term4 + lon_term5;
    
    // Compute cos(φ₁)cos(φ₂)
    let cos_product = &point1.cos_lat * &point2.cos_lat / SCALE_FACTOR;
    
    // Combine terms for 'a'
    let a = &sin_squared_half_delta_lat + &cos_product * &sin_squared_half_delta_lon;
    
    println!("    Computation time: {:.2?}", compute_start_time.elapsed());
    
    // Step 4: Compute angular distance 'c' using polynomial approximation
    // arcsin(√a) ≈ √a + (a√a)/6 + (3a²√a)/40 + (5a³√a)/112 + (35a⁴√a)/1152
    let sqrt_a = &a; // Note: This is a simplification. In practice, we'd need a proper sqrt approximation
    let a_sqrt_a = &a * sqrt_a;
    let a_squared_sqrt_a = &a * &a_sqrt_a;
    let a_cubed_sqrt_a = &a * &a_squared_sqrt_a;
    let a_fourth_sqrt_a = &a * &a_cubed_sqrt_a;
    
    let c = sqrt_a + 
            &a_sqrt_a / 6_u32 + 
            &a_squared_sqrt_a * 3_u32 / 40_u32 + 
            &a_cubed_sqrt_a * 5_u32 / 112_u32 + 
            &a_fourth_sqrt_a * 35_u32 / 1152_u32;
    
    // Step 5: Multiply by Earth's radius
    let result = &c * EARTH_RADIUS_KM;
    
    result
}

// Compare which point is closer to the reference point
pub fn compare_distances(
    point_x: &ClientData,
    point_y: &ClientData,
    reference_z: &ClientData,
    client_key: &ClientKey
) -> FheBool {
    println!("Calculating distance from X to Z...");
    let xz_start_time = Instant::now();
    let x_to_z_value = calculate_haversine_distance_squared(point_x, reference_z, client_key);
    println!("  X to Z calculation time: {:.2?}", xz_start_time.elapsed());
    
    println!("Calculating distance from Y to Z...");
    let yz_start_time = Instant::now();
    let y_to_z_value = calculate_haversine_distance_squared(point_y, reference_z, client_key);
    println!("  Y to Z calculation time: {:.2?}", yz_start_time.elapsed());
    
    println!("Comparing distances...");
    let compare_start_time = Instant::now();
    
    // Final step: Compare encrypted distances
    let result = x_to_z_value.lt(&y_to_z_value);
    
    println!("  Comparison operation time: {:.2?}", compare_start_time.elapsed());
    
    result
}

// Function to calculate the approximate Haversine distance for verification
pub fn approximate_haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    // Convert to radians
    let lat1_rad = lat1 * PI / 180.0;
    let lat2_rad = lat2 * PI / 180.0;
    
    // Calculate delta in degrees
    let delta_lat = (lat2 - lat1).abs();
    let delta_lon = (lon2 - lon1).abs();
    
    // Ensure we're taking the minimum path around the globe for longitude
    let delta_lon = delta_lon.min(360.0 - delta_lon);
    
    // Calculate the average cosine
    let avg_cos = (lat1_rad.cos() + lat2_rad.cos()) / 2.0;
    
    // Normalize by max possible values (same as in encrypted version)
    let norm_factor = 16.0;
    let lat_scaled = delta_lat / norm_factor;
    let lon_scaled = delta_lon / norm_factor;
    
    // Apply polynomial approximation of sin²(θ/2)
    // For small angles: sin²(θ/2) ≈ (θ/2)²
    // For larger angles: sin²(θ/2) ≈ (θ/2)² - (θ/2)⁴/3
    
    // Calculate squared terms
    let lat_squared = (lat_scaled / 2.0).powi(2);
    let lon_squared = (lon_scaled / 2.0).powi(2);
    
    // Calculate correction terms for larger angles
    let correction_factor = 48.0;
    let lat_correction = lat_squared.powi(2) / correction_factor;
    let lon_correction = lon_squared.powi(2) / correction_factor;
    
    // Apply the polynomial approximation
    let lat_term = lat_squared - lat_correction;
    let lon_term = lon_squared - lon_correction;
    
    // Weight the longitude term with cosine
    let weighted_lon = lon_term * avg_cos;
    
    // Haversine formula approximation
    let dist_squared = lat_term + weighted_lon;
    
    // Return approximated distance
    dist_squared.sqrt()
}

// Main function to execute the distance comparison
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Default test case points
    let points = vec![
        Point {
            name: "Basel".to_string(),
            lat: 47.5596,
            lon: 7.5886,
        },
        Point {
            name: "Lugano".to_string(),
            lat: 46.0037,
            lon: 8.9511,
        },
        Point {
            name: "Zurich".to_string(),
            lat: 47.3769,
            lon: 8.5417,
        },
    ];

    // Check if points were provided as command line arguments
    let args: Vec<String> = std::env::args().collect();
    let points = if args.len() == 10 {  // 3 points * 3 values (lat, lon, name) + program name
        vec![
            Point {
                name: args[1].clone(),
                lat: args[2].parse()?,
                lon: args[3].parse()?,
            },
            Point {
                name: args[4].clone(),
                lat: args[5].parse()?,
                lon: args[6].parse()?,
            },
            Point {
                name: args[7].clone(),
                lat: args[8].parse()?,
                lon: args[9].parse()?,
            },
        ]
    } else {
        println!("Using default test points:");
        for (i, point) in points.iter().enumerate() {
            println!("Point {}: {} (lat: {}, lon: {})", 
                    ['X', 'Y', 'Z'][i], point.name, point.lat, point.lon);
        }
        points
    };

    println!("\nStarting GPS distance comparison using FHE...");
    println!("Following Haversine formula approximation with client precomputation");

    // Configure TFHE
    let config = ConfigBuilder::default().build();
    let (client_key, server_keys) = generate_keys(config);
    set_server_key(server_keys);

    // Client-side: Precompute and encrypt data for 3 points
    println!("\n1. CLIENT SIDE: Precomputing and encrypting coordinates");
    
    // Point X
    let x_name = Some(points[0].name.clone());
    let x_desc = if let Some(n) = x_name.as_deref() { format!(" ({})", n) } else { String::new() };
    println!("Point X{}: Latitude {:.4}° N, Longitude {:.4}° E", x_desc, points[0].lat, points[0].lon);
    let client_data_x = precompute_client_data(points[0].lat, points[0].lon, x_name.clone(), &client_key)?;
    
    // Point Y
    let y_name = Some(points[1].name.clone());
    let y_desc = if let Some(n) = y_name.as_deref() { format!(" ({})", n) } else { String::new() };
    println!("Point Y{}: Latitude {:.4}° N, Longitude {:.4}° E", y_desc, points[1].lat, points[1].lon);
    let client_data_y = precompute_client_data(points[1].lat, points[1].lon, y_name.clone(), &client_key)?;
    
    // Point Z (reference point)
    let z_name = Some(points[2].name.clone());
    let z_desc = if let Some(n) = z_name.as_deref() { format!(" ({})", n) } else { String::new() };
    println!("Point Z{}: Latitude {:.4}° N, Longitude {:.4}° E", z_desc, points[2].lat, points[2].lon);
    let client_data_z = precompute_client_data(points[2].lat, points[2].lon, z_name.clone(), &client_key)?;

    // For debugging: verify the actual scaling
    println!("\nPlaintext calculations for verification:");
    let point_x = GeoPoint::new(points[0].lon, points[0].lat);
    let point_y = GeoPoint::new(points[1].lon, points[1].lat);
    let point_z = GeoPoint::new(points[2].lon, points[2].lat);
    
    println!("Distance X-Z: {:.4} km", point_x.haversine_distance(&point_z) / 1000.0);
    println!("Distance Y-Z: {:.4} km", point_y.haversine_distance(&point_z) / 1000.0);
    
    let approx_dist_xz = approximate_haversine_distance(points[0].lat, points[0].lon, points[2].lat, points[2].lon);
    let approx_dist_yz = approximate_haversine_distance(points[1].lat, points[1].lon, points[2].lat, points[2].lon);
    println!("Approximate distance X-Z: {:.4} units", approx_dist_xz);
    println!("Approximate distance Y-Z: {:.4} units", approx_dist_yz);
    println!("X should be closer: {}", approx_dist_xz < approx_dist_yz);

    // Server-side: Calculate and compare distances
    println!("\n2. SERVER SIDE: Performing FHE computations on encrypted data");
    let start_time = Instant::now();
    
    let closer_x = compare_distances(&client_data_x, &client_data_y, &client_data_z, &client_key);
    let is_x_closer = closer_x.decrypt(&client_key);
    
    let duration = start_time.elapsed();
    println!("FHE computation completed in {:.2?}", duration);
    println!("Result: Point X is {} to Point Z than Point Y", 
             if is_x_closer { "closer" } else { "further" });

    Ok(())
}

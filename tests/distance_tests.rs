use tfhe::prelude::*;
use tfhe::{generate_keys, set_server_key, ConfigBuilder};
use std::time::Instant;
use tfhe_gps_distance::*;

// Shared test utility functions
fn run_test_case(point_x: Point, point_y: Point, point_z: Point) -> (bool, f64, f64, std::time::Duration) {
    println!("\nRunning test case:");
    println!("Point X ({}): {:.4}° N, {:.4}° E", point_x.name, point_x.lat, point_x.lon);
    println!("Point Y ({}): {:.4}° N, {:.4}° E", point_y.name, point_y.lat, point_y.lon);
    println!("Point Z ({}): {:.4}° N, {:.4}° E", point_z.name, point_z.lat, point_z.lon);
    
    let total_start_time = Instant::now();
    
    // Configure TFHE
    let config_start_time = Instant::now();
    let config = ConfigBuilder::default().build();
    let (client_key, server_keys) = generate_keys(config);
    set_server_key(server_keys);
    println!("TFHE configuration time: {:.2?}", config_start_time.elapsed());

    // Precompute and encrypt data
    let encrypt_start_time = Instant::now();
    let client_data_x = precompute_client_data(point_x.lat, point_x.lon, Some(point_x.name.clone()), &client_key).unwrap();
    let client_data_y = precompute_client_data(point_y.lat, point_y.lon, Some(point_y.name.clone()), &client_key).unwrap();
    let client_data_z = precompute_client_data(point_z.lat, point_z.lon, Some(point_z.name.clone()), &client_key).unwrap();
    println!("Encryption time: {:.2?}", encrypt_start_time.elapsed());

    // Calculate distances
    let dist_start_time = Instant::now();
    let dist_xz = haversine_distance(point_x.lat, point_x.lon, point_z.lat, point_z.lon);
    let dist_yz = haversine_distance(point_y.lat, point_y.lon, point_z.lat, point_z.lon);
    println!("Plaintext distance calculation time: {:.2?}", dist_start_time.elapsed());

    // Compare distances using FHE
    let fhe_start_time = Instant::now();
    let closer_x = compare_distances(&client_data_x, &client_data_y, &client_data_z, &client_key);
    println!("FHE comparison time: {:.2?}", fhe_start_time.elapsed());
    
    let decrypt_start_time = Instant::now();
    let is_x_closer = closer_x.decrypt(&client_key);
    println!("Decryption time: {:.2?}", decrypt_start_time.elapsed());

    let total_duration = total_start_time.elapsed();
    println!("\nTest results:");
    println!("Actual distances:");
    println!("  X-Z: {:.2} km", dist_xz);
    println!("  Y-Z: {:.2} km", dist_yz);
    println!("FHE comparison result: Point X is {} to Point Z than Point Y", 
            if is_x_closer { "closer" } else { "further" });
    println!("Total execution time: {:.2?}", total_duration);

    (is_x_closer, dist_xz, dist_yz, total_duration)
}

#[test]
fn test_swiss_cities() {
    let point_x = Point {
        name: "Basel".to_string(),
        lat: 47.5596,
        lon: 7.5886,
    };
    let point_y = Point {
        name: "Lugano".to_string(),
        lat: 46.0037,
        lon: 8.9511,
    };
    let point_z = Point {
        name: "Zurich".to_string(),
        lat: 47.3769,
        lon: 8.5417,
    };

    let (is_x_closer, dist_xz, dist_yz, duration) = run_test_case(point_x, point_y, point_z);
    
    assert!(is_x_closer, "Basel should be closer to Zurich than Lugano");
    assert!(dist_xz < dist_yz, "Distance Basel-Zurich should be less than Lugano-Zurich");
    assert!((dist_xz - 74.47).abs() < 0.1, "Distance Basel-Zurich should be approximately 74.47 km");
    assert!((dist_yz - 155.85).abs() < 0.1, "Distance Lugano-Zurich should be approximately 155.85 km");
    println!("Test completed in {:.2?}", duration);
}

#[test]
fn test_near_points() {
    let point_x = Point {
        name: "Point1".to_string(),
        lat: 47.3769,
        lon: 8.5418,
    };
    let point_y = Point {
        name: "Point2".to_string(),
        lat: 47.3769,
        lon: 8.5417,
    };
    let point_z = Point {
        name: "Reference".to_string(),
        lat: 47.3769,
        lon: 8.5419,
    };

    let (is_x_closer, dist_xz, dist_yz, duration) = run_test_case(point_x, point_y, point_z);
    
    assert!(is_x_closer, "Point1 should be closer to Reference than Point2");
    assert!(dist_xz < dist_yz, "Distance Point1-Reference should be less than Point2-Reference");
    println!("Test completed in {:.2?}", duration);
}

#[test]
fn test_far_points() {
    let point_x = Point {
        name: "Tokyo".to_string(),
        lat: 35.6762,
        lon: 139.6503,
    };
    let point_y = Point {
        name: "NewYork".to_string(),
        lat: 40.7128,
        lon: -74.0060,
    };
    let point_z = Point {
        name: "London".to_string(),
        lat: 51.5074,
        lon: -0.1278,
    };

    let (is_x_closer, dist_xz, dist_yz, duration) = run_test_case(point_x, point_y, point_z);
    
    assert!(!is_x_closer, "New York should be closer to London than Tokyo");
    assert!(dist_yz < dist_xz, "Distance NewYork-London should be less than Tokyo-London");
    println!("Test completed in {:.2?}", duration);
}

#[test]
fn test_equator_points() {
    let point_x = Point {
        name: "Quito".to_string(),
        lat: 0.0,
        lon: -78.4678,
    };
    let point_y = Point {
        name: "Singapore".to_string(),
        lat: 0.0,
        lon: 103.8198,
    };
    let point_z = Point {
        name: "Reference".to_string(),
        lat: 0.0,
        lon: 0.0,
    };

    let (is_x_closer, dist_xz, dist_yz, duration) = run_test_case(point_x, point_y, point_z);
    
    assert!(is_x_closer, "Quito should be closer to Reference than Singapore");
    assert!(dist_xz < dist_yz, "Distance Quito-Reference should be less than Singapore-Reference");
    println!("Test completed in {:.2?}", duration);
} 
use tfhe::prelude::*;
use tfhe::{generate_keys, set_server_key, ConfigBuilder, FheUint32};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting... Determining which point is closer to point Z...");

    // Configure TFHE for homomorphic integer encryption
    let config = ConfigBuilder::default().build();

    // Generate client and server keys
    let (client_key, server_keys) = generate_keys(config);

    // Set server key for performing operations on encrypted data
    set_server_key(server_keys);

    // Example GPS coordinates for points X, Y, and Z (scaled by 10,000)
    // Basel, Switzerland (Point X)
    let x_lat = (47.5596 * 10_000.0) as u32; // Latitude of point X (47.5596° N)
    let x_lon = (7.5886 * 10_000.0) as u32;  // Longitude of point X (7.5886° E)
    println!("Point X (Basel): Latitude 47.5596° N, Longitude 7.5886° E");

    // Lugano, Switzerland (Point Y)
    let y_lat = (46.0037 * 10_000.0) as u32; // Latitude of point Y (46.0037° N)
    let y_lon = (8.9511 * 10_000.0) as u32;  // Longitude of point Y (8.9511° E)
    println!("Point Y (Lugano): Latitude 46.0037° N, Longitude 8.9511° E");

    // Zurich, Switzerland (Point Z)
    let z_lat = (47.3769 * 10_000.0) as u32; // Latitude of point Z (47.3769° N)
    let z_lon = (8.5417 * 10_000.0) as u32;  // Longitude of point Z (8.5417° E)
    println!("Point Z (Zurich): Latitude 47.3769° N, Longitude 8.5417° E");

    // Encrypt the coordinates using the client key
    let encrypted_x_lat = FheUint32::try_encrypt(x_lat, &client_key)?;
    let encrypted_x_lon = FheUint32::try_encrypt(x_lon, &client_key)?;
    let encrypted_y_lat = FheUint32::try_encrypt(y_lat, &client_key)?;
    let encrypted_y_lon = FheUint32::try_encrypt(y_lon, &client_key)?;
    let encrypted_z_lat = FheUint32::try_encrypt(z_lat, &client_key)?;
    let encrypted_z_lon = FheUint32::try_encrypt(z_lon, &client_key)?;

    println!("Everything is encrypted. Let's start the computation...");

    // Start timing the main computation
    let start_time = Instant::now();

    // Compute squared Euclidean distance from X to Z: (x_lat - z_lat)^2 + (x_lon - z_lon)^2
    let dx_z = &encrypted_x_lat - &encrypted_z_lat;
    let dy_z = &encrypted_x_lon - &encrypted_z_lon;
    let dx_z2 = &dx_z * &dx_z;
    let dy_z2 = &dy_z * &dy_z;
    let distance_xz = &dx_z2 + &dy_z2;

    // Compute squared Euclidean distance from Y to Z: (y_lat - z_lat)^2 + (y_lon - z_lon)^2
    let dy_z_y = &encrypted_y_lat - &encrypted_z_lat;
    let dx_z_y = &encrypted_y_lon - &encrypted_z_lon;
    let dx_z_y2 = &dy_z_y * &dy_z_y;
    let dy_z_y2 = &dx_z_y * &dx_z_y;
    let distance_yz = &dx_z_y2 + &dy_z_y2;

    // Compare distances homomorphically to determine which is smaller
    let closer_x = &distance_xz.lt(&distance_yz); // true if X is closer, false if Y is closer

    // Stop timing the computation
    let duration = start_time.elapsed();

    // Decrypt results to determine the closer point
    let is_x_closer: bool = closer_x.decrypt(&client_key);

    if is_x_closer {
        println!("Point X (Basel) is closer to point Z (Zurich).");
    } else {
        println!("Point Y (Lugano) is closer to point Z (Zurich).");
    }

    // Print the computation duration (excluding key generation)
    println!("Computation time (excluding key generation): {:?}", duration);

    Ok(())
}

// Re-export the public interface

// Include the main module with public exports
#[path = "main.rs"]
mod main_mod;

// Re-export the public items from the main module
pub use main_mod::{
    SCALE_FACTOR, 
    EARTH_RADIUS_KM, 
    Point, 
    ClientData, 
    precompute_client_data, 
    calculate_haversine_distance_squared, 
    compare_distances, 
    approximate_haversine_distance
}; 
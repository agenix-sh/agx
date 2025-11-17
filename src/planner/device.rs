use candle_core::Device;

use super::types::ModelError;

/// GPU compute capability version
#[derive(Debug, Clone, Copy)]
pub struct ComputeCapability {
    pub major: u32,
    pub minor: u32,
}

/// Device selector for automatic GPU detection
pub struct DeviceSelector;

impl DeviceSelector {
    /// Auto-detect the best available device
    ///
    /// Priority order: CUDA > Metal > CPU
    ///
    /// # Returns
    /// The best available `Device`, or an error if device initialization fails
    pub fn auto_select() -> Result<Device, ModelError> {
        // Try CUDA first (if available)
        #[cfg(feature = "cuda")]
        {
            if Self::cuda_is_available() {
                match Self::verify_cuda() {
                    Ok(_) => {
                        log::info!("Selected CUDA device");
                        return Device::new_cuda(0).map_err(|e| {
                            ModelError::ConfigError(format!("Failed to initialize CUDA: {}", e))
                        });
                    }
                    Err(e) => {
                        log::warn!("CUDA verification failed: {}", e);
                    }
                }
            }
        }

        // Try Metal (macOS GPU)
        #[cfg(feature = "metal")]
        {
            if Self::metal_is_available() {
                log::info!("Selected Metal device");
                return Device::new_metal(0).map_err(|e| {
                    ModelError::ConfigError(format!("Failed to initialize Metal: {}", e))
                });
            }
        }

        // Fallback to CPU
        log::warn!("No GPU detected, falling back to CPU");
        Ok(Device::Cpu)
    }

    /// Check if CUDA is available
    #[cfg(feature = "cuda")]
    fn cuda_is_available() -> bool {
        // Use candle's built-in CUDA detection
        matches!(Device::new_cuda(0), Ok(_))
    }

    /// Check if Metal is available
    #[cfg(feature = "metal")]
    fn metal_is_available() -> bool {
        // Use candle's built-in Metal detection
        matches!(Device::new_metal(0), Ok(_))
    }

    /// Verify CUDA installation and log version information
    #[cfg(feature = "cuda")]
    fn verify_cuda() -> Result<(), ModelError> {
        // Note: Candle 0.9 has alpha CUDA support
        // This is a placeholder for future CUDA version detection
        // For now, we just log a warning about alpha support
        log::warn!("Candle CUDA support is in alpha. CUDA 12.0+ recommended for best compatibility.");

        // TODO: When Candle exposes CUDA version APIs, add:
        // - CUDA version detection
        // - Compute capability detection (especially for Blackwell 10.x)
        // - Memory availability checks

        Ok(())
    }
}

/// Select device from environment variable or auto-detect
///
/// Respects `AGX_DEVICE` environment variable:
/// - `cuda` - Force CUDA
/// - `metal` - Force Metal
/// - `cpu` - Force CPU
/// - Not set or invalid - Auto-detect
pub fn select_device_from_env() -> Result<Device, ModelError> {
    match std::env::var("AGX_DEVICE") {
        Ok(dev) => {
            let normalized = dev.to_lowercase();
            match normalized.as_str() {
                "cuda" => {
                    #[cfg(feature = "cuda")]
                    {
                        log::info!("Device selection: CUDA (forced by AGX_DEVICE)");
                        return Device::new_cuda(0).map_err(|e| {
                            ModelError::ConfigError(format!(
                                "Failed to initialize CUDA (forced by AGX_DEVICE): {}",
                                e
                            ))
                        });
                    }
                    #[cfg(not(feature = "cuda"))]
                    {
                        return Err(ModelError::ConfigError(
                            "CUDA requested but not compiled with cuda feature".to_string(),
                        ));
                    }
                }
                "metal" => {
                    #[cfg(feature = "metal")]
                    {
                        log::info!("Device selection: Metal (forced by AGX_DEVICE)");
                        return Device::new_metal(0).map_err(|e| {
                            ModelError::ConfigError(format!(
                                "Failed to initialize Metal (forced by AGX_DEVICE): {}",
                                e
                            ))
                        });
                    }
                    #[cfg(not(feature = "metal"))]
                    {
                        return Err(ModelError::ConfigError(
                            "Metal requested but not compiled with metal feature".to_string(),
                        ));
                    }
                }
                "cpu" => {
                    log::info!("Device selection: CPU (forced by AGX_DEVICE)");
                    return Ok(Device::Cpu);
                }
                _ => {
                    log::warn!(
                        "Invalid AGX_DEVICE value: '{}', falling back to auto-detect",
                        dev
                    );
                    DeviceSelector::auto_select()
                }
            }
        }
        Err(_) => DeviceSelector::auto_select(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_auto_selection() {
        // Should not panic and should return a valid device
        let device = DeviceSelector::auto_select();
        assert!(device.is_ok());
    }

    #[test]
    fn test_cpu_device_from_env() {
        // Set environment variable to force CPU
        std::env::set_var("AGX_DEVICE", "cpu");
        let device = select_device_from_env().unwrap();
        assert!(matches!(device, Device::Cpu));
        std::env::remove_var("AGX_DEVICE");
    }

    #[test]
    fn test_invalid_device_from_env() {
        // Invalid value should fall back to auto-detect
        std::env::set_var("AGX_DEVICE", "invalid");
        let device = select_device_from_env();
        assert!(device.is_ok()); // Should succeed with auto-detect
        std::env::remove_var("AGX_DEVICE");
    }

    #[cfg(feature = "metal")]
    #[test]
    fn test_metal_device() {
        // On macOS with metal feature, Metal should be available
        let device = Device::new_metal(0);
        // This may fail on non-macOS systems, but that's expected
        if device.is_ok() {
            assert!(DeviceSelector::metal_is_available());
        }
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_cuda_device() {
        // On systems with CUDA, this should work
        let device = Device::new_cuda(0);
        if device.is_ok() {
            assert!(DeviceSelector::cuda_is_available());
        }
    }
}

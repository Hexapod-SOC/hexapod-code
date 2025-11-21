//! Serial port interface for LiDAR communication

use anyhow::Result;

#[cfg(feature = "real")]
use anyhow::Context;

#[cfg(feature = "real")]
use std::os::unix::io::RawFd;

/// Serial interface for LiDAR
pub struct SerialInterface {
    #[cfg(feature = "real")]
    fd: RawFd,
    #[cfg(feature = "dummy")]
    _port_name: String,
}

impl SerialInterface {
    /// Open a serial port connection to the LiDAR
    #[cfg(feature = "real")]
    pub fn new(port_name: &str) -> Result<Self> {
        use std::ffi::CString;

        let path = CString::new(port_name)
            .context("Invalid port name")?;

        // Open the serial port
        let fd = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
            )
        };

        if fd < 0 {
            return Err(anyhow::anyhow!("Failed to open serial port: {}", port_name));
        }

        // Configure the serial port
        unsafe {
            let mut options: libc::termios = std::mem::zeroed();
            
            // Get current options
            if libc::tcgetattr(fd, &mut options) != 0 {
                libc::close(fd);
                return Err(anyhow::anyhow!("Failed to get serial port attributes"));
            }

            // Set baud rate to 230400
            libc::cfsetispeed(&mut options, libc::B230400);
            libc::cfsetospeed(&mut options, libc::B230400);

            // 8N1 mode, no flow control
            options.c_cflag |= libc::CLOCAL | libc::CREAD | libc::CS8;
            options.c_cflag &= !(libc::CSTOPB | libc::PARENB);
            
            // Raw input
            options.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ECHOE | libc::ISIG);
            options.c_oflag &= !libc::OPOST;
            options.c_iflag &= !(libc::IXON | libc::IXOFF | libc::INLCR | libc::IGNCR | libc::ICRNL);

            // Timeout settings
            options.c_cc[libc::VMIN] = 0;
            options.c_cc[libc::VTIME] = 1; // 100ms timeout

            // Apply settings
            if libc::tcsetattr(fd, libc::TCSANOW, &options) != 0 {
                libc::close(fd);
                return Err(anyhow::anyhow!("Failed to set serial port attributes"));
            }

            // Flush buffers
            libc::tcflush(fd, libc::TCIFLUSH);
        }

        Ok(Self { fd })
    }

    /// Dummy implementation for testing
    #[cfg(feature = "dummy")]
    pub fn new(port_name: &str) -> Result<Self> {
        println!("[LiDAR Dummy] Opening port: {}", port_name);
        Ok(Self {
            _port_name: port_name.to_string(),
        })
    }

    /// Read data from the serial port
    #[cfg(feature = "real")]
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize> {
        let n = unsafe {
            libc::read(
                self.fd,
                buffer.as_mut_ptr() as *mut libc::c_void,
                buffer.len(),
            )
        };

        if n < 0 {
            // Check if it's just "would block" (no data available)
            let errno = unsafe { *libc::__errno_location() };
            if errno == libc::EAGAIN || errno == libc::EWOULDBLOCK {
                // No data available right now, not an error
                Ok(0)
            } else {
                Err(anyhow::anyhow!("Failed to read from serial port: errno {}", errno))
            }
        } else {
            Ok(n as usize)
        }
    }

    /// Dummy read implementation
    #[cfg(feature = "dummy")]
    pub fn read(&mut self, _buffer: &mut [u8]) -> Result<usize> {
        // Simulate no data available
        std::thread::sleep(std::time::Duration::from_millis(10));
        Ok(0)
    }

    /// Write data to the serial port (for commands)
    #[cfg(feature = "real")]
    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        let n = unsafe {
            libc::write(
                self.fd,
                data.as_ptr() as *const libc::c_void,
                data.len(),
            )
        };

        if n < 0 {
            Err(anyhow::anyhow!("Failed to write to serial port"))
        } else {
            Ok(n as usize)
        }
    }

    /// Dummy write implementation
    #[cfg(feature = "dummy")]
    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        println!("[LiDAR Dummy] Writing {} bytes", data.len());
        Ok(data.len())
    }

    /// Flush the serial port buffers
    #[cfg(feature = "real")]
    pub fn flush(&mut self) -> Result<()> {
        unsafe {
            if libc::tcdrain(self.fd) != 0 {
                return Err(anyhow::anyhow!("Failed to flush serial port"));
            }
        }
        Ok(())
    }

    /// Dummy flush implementation
    #[cfg(feature = "dummy")]
    pub fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    /// Clear the input buffer
    #[cfg(feature = "real")]
    pub fn clear_input_buffer(&mut self) -> Result<()> {
        unsafe {
            if libc::tcflush(self.fd, libc::TCIFLUSH) != 0 {
                return Err(anyhow::anyhow!("Failed to clear input buffer"));
            }
        }
        Ok(())
    }

    /// Dummy clear implementation
    #[cfg(feature = "dummy")]
    pub fn clear_input_buffer(&mut self) -> Result<()> {
        Ok(())
    }

    /// Clear the output buffer
    #[cfg(feature = "real")]
    pub fn clear_output_buffer(&mut self) -> Result<()> {
        unsafe {
            if libc::tcflush(self.fd, libc::TCOFLUSH) != 0 {
                return Err(anyhow::anyhow!("Failed to clear output buffer"));
            }
        }
        Ok(())
    }

    /// Dummy clear implementation
    #[cfg(feature = "dummy")]
    pub fn clear_output_buffer(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "real")]
impl Drop for SerialInterface {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_dummy_serial() {
        #[cfg(feature = "dummy")]
        {
            use super::SerialInterface;
            let mut serial = SerialInterface::new("/dev/ttyUSB0").unwrap();
            let mut buffer = [0u8; 100];
            let n = serial.read(&mut buffer).unwrap();
            assert_eq!(n, 0); // Dummy returns 0
        }
    }
}

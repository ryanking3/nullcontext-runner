use std::str;
use zeroize::Zeroize;

#[derive(Debug)]
pub struct SensitiveBytes {
    value: Vec<u8>,
}

impl SensitiveBytes {
    pub fn new(value: String) -> Self {
        Self {
            value: value.into_bytes(),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.value
    }

    pub fn as_str(&self) -> &str {
        str::from_utf8(&self.value).expect("SensitiveBytes contained invalid UTF-8")
    }

    pub fn sanitize(&mut self) {
        self.value.zeroize();
    }
}

impl Drop for SensitiveBytes {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

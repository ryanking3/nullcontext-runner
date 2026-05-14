use zeroize::Zeroize;

#[derive(Debug)]
pub struct SensitiveString {
    value: String,
}

impl SensitiveString {
    pub fn new(value: String) -> Self {
        Self { value }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn sanitize(&mut self) {
        self.value.zeroize();
    }
}

impl Drop for SensitiveString {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

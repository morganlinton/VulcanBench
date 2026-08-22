use std::io::Write;

/// Abstract over your TOML serialization
pub trait Encoder {
    fn encode(&self, data: crate::decoded::DecodedValue) -> Result<String, crate::Error>;

    fn verify_valid_case(&self, decoded: &[u8], fixture: &dyn Decoder) -> Result<(), crate::Error> {
        let decoded_expected = crate::decoded::DecodedValue::from_slice(decoded)?;
        let actual = self.encode(decoded_expected.clone())?;
        let decoded_actual = fixture.decode(actual.as_bytes()).map_err(|err| {
            crate::Error::new(format!(
                "Could not parse encoded TOML: {err}\n```\n{actual}\n```"
            ))
        })?;

        if decoded_actual == decoded_expected {
            Ok(())
        } else {
            Err(crate::Error::new(format!(
                "Unexpected decoding\n```toml\n{}\n```\nExpected\n{}\nActual\n{}",
                actual,
                decoded_expected.to_string_pretty().unwrap(),
                decoded_actual.to_string_pretty().unwrap()
            )))
        }
    }

    fn name(&self) -> &str;
}

/// Abstract over your TOML deserialization
pub trait Decoder {
    fn decode(&self, data: &[u8]) -> Result<crate::decoded::DecodedValue, crate::Error>;

    fn verify_valid_case(&self, fixture: &[u8], expected: &[u8]) -> Result<(), crate::Error> {
        let actual = self.decode(fixture)?;
        let expected = crate::decoded::DecodedValue::from_slice(expected)?;
        if actual == expected {
            Ok(())
        } else {
            Err(crate::Error::new(format!(
                "Unexpected decoding\n```toml\n{}\n```\nExpected\n{}\nActual\n{}",
                std::str::from_utf8(fixture).unwrap(),
                expected.to_string_pretty().unwrap(),
                actual.to_string_pretty().unwrap()
            )))
        }
    }

    fn verify_invalid_case(&self, fixture: &[u8]) -> Result<crate::Error, crate::Error> {
        match self.decode(fixture) {
            Ok(value) => Err(crate::Error::new(format!(
                "Should have failed but got:\n{}\n```toml\n{}\n```",
                value.to_string_pretty().unwrap(),
                std::str::from_utf8(fixture).unwrap(),
            ))),
            Err(err) => Ok(err),
        }
    }

    fn name(&self) -> &str;
}

/// TOML parser-as-a-binary
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    bin: std::path::PathBuf,
}

impl Command {
    pub fn new(path: impl AsRef<std::path::Path>) -> Self {
        Self {
            bin: path.as_ref().to_owned(),
        }
    }
}

impl Encoder for Command {
    fn encode(&self, data: crate::decoded::DecodedValue) -> Result<String, crate::Error> {
        let data = data.to_string_pretty()?;

        let mut cmd = std::process::Command::new(&self.bin);
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let child = cmd.spawn().map_err(crate::Error::new)?;
        child
            .stdin
            .as_ref()
            .unwrap()
            .write_all(data.as_bytes())
            .map_err(crate::Error::new)?;

        let output = child.wait_with_output().map_err(crate::Error::new)?;
        if output.status.success() {
            let output = String::from_utf8(output.stdout).map_err(crate::Error::new)?;
            Ok(output)
        } else {
            let message = String::from_utf8_lossy(&output.stderr);
            Err(crate::Error::new(format!(
                "{} failed with {:?}: {}",
                self.bin.display(),
                output.status.code(),
                message
            )))
        }
    }

    fn name(&self) -> &str {
        self.bin.to_str().expect("we'll always get valid UTF-8")
    }
}

impl Decoder for Command {
    fn decode(&self, data: &[u8]) -> Result<crate::decoded::DecodedValue, crate::Error> {
        let mut cmd = std::process::Command::new(&self.bin);
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let child = cmd.spawn().map_err(crate::Error::new)?;
        child
            .stdin
            .as_ref()
            .unwrap()
            .write_all(data)
            .map_err(crate::Error::new)?;

        let output = child.wait_with_output().map_err(crate::Error::new)?;
        if output.status.success() {
            let output = crate::decoded::DecodedValue::from_slice(&output.stdout)
                .map_err(crate::Error::new)?;
            Ok(output)
        } else {
            let message = String::from_utf8_lossy(&output.stderr);
            Err(crate::Error::new(format!(
                "{} failed with {:?}: {}",
                self.bin.display(),
                output.status.code(),
                message
            )))
        }
    }

    fn name(&self) -> &str {
        self.bin.to_str().expect("we'll always get valid UTF-8")
    }
}

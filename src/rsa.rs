/// RSASSA-PKCS1-v1_5 using SHA-256.
use openssl::{
    bn::BigNum,
    hash::MessageDigest,
    pkey::{Id, PKey, Private, Public},
    rsa::{Padding, Rsa},
    sign::{RsaPssSaltlen, Signer, Verifier},
};
use smallvec::SmallVec;

use crate::{
    jwk::Jwk, url_safe_trailing_bits, Error, PrivateKeyToJwk, PublicKeyToJwk, Result, SigningKey,
    VerificationKey,
};

/// RSA signature algorithms.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RsaAlgorithm {
    RS256,
    RS384,
    RS512,
    PS256,
    PS384,
    PS512,
}

impl RsaAlgorithm {
    pub fn is_pss(self) -> bool {
        matches!(
            self,
            RsaAlgorithm::PS256 | RsaAlgorithm::PS384 | RsaAlgorithm::PS512
        )
    }

    fn digest(self) -> MessageDigest {
        use RsaAlgorithm::*;
        match self {
            RS256 | PS256 => MessageDigest::sha256(),
            RS384 | PS384 => MessageDigest::sha384(),
            RS512 | PS512 => MessageDigest::sha512(),
        }
    }

    pub fn name(self) -> &'static str {
        use RsaAlgorithm::*;
        match self {
            RS256 => "RS256",
            RS384 => "RS384",
            RS512 => "RS512",
            PS256 => "PS256",
            PS384 => "PS384",
            PS512 => "PS512",
        }
    }

    pub fn from_name(name: &str) -> Result<Self> {
        Ok(match name {
            "RS256" => RsaAlgorithm::RS256,
            "RS384" => RsaAlgorithm::RS384,
            "RS512" => RsaAlgorithm::RS512,
            "PS256" => RsaAlgorithm::PS256,
            "PS384" => RsaAlgorithm::PS384,
            "PS512" => RsaAlgorithm::PS512,
            _ => return Err(Error::UnsupportedOrInvalidKey),
        })
    }
}

/// RSA Private Key.
///
/// By default, it only verifies signatures generated by the same algorithm used
/// for signing. If you want to verify signatures generated by any RSA
/// algorithm, set `verify_any` to `true`.
#[derive(Debug, Clone)]
pub struct RsaPrivateKey {
    private_key: PKey<Private>,
    pub algorithm: RsaAlgorithm,
    pub verify_any: bool,
}

impl RsaPrivateKey {
    /// bits >= 2048.
    pub fn generate(bits: u32, algorithm: RsaAlgorithm) -> Result<Self> {
        if bits < 2048 {
            return Err(Error::UnsupportedOrInvalidKey);
        }

        Ok(Self {
            private_key: PKey::from_rsa(Rsa::generate(bits)?)?,
            algorithm,
            verify_any: false,
        })
    }

    pub(crate) fn from_pkey(pkey: PKey<Private>, algorithm: RsaAlgorithm) -> Result<Self> {
        if pkey.bits() < 2048 || !pkey.rsa()?.check_key()? {
            return Err(Error::UnsupportedOrInvalidKey);
        }
        Ok(Self {
            private_key: pkey,
            algorithm,
            verify_any: false,
        })
    }

    pub(crate) fn from_pkey_without_check(
        pkey: PKey<Private>,
        algorithm: RsaAlgorithm,
    ) -> Result<Self> {
        if pkey.bits() < 2048 {
            return Err(Error::UnsupportedOrInvalidKey);
        }
        Ok(Self {
            private_key: pkey,
            algorithm,
            verify_any: false,
        })
    }

    pub fn from_pem(pem: &[u8], algorithm: RsaAlgorithm) -> Result<Self> {
        let pk = PKey::private_key_from_pem(pem)?;
        Self::from_pkey(pk, algorithm)
    }

    pub fn private_key_to_pem_pkcs8(&self) -> Result<String> {
        Ok(String::from_utf8(
            self.private_key.private_key_to_pem_pkcs8()?,
        )?)
    }

    pub fn public_key_to_pem(&self) -> Result<String> {
        Ok(String::from_utf8(self.private_key.public_key_to_pem()?)?)
    }

    pub fn public_key_to_pem_pkcs1(&self) -> Result<String> {
        Ok(String::from_utf8(
            self.private_key.rsa()?.public_key_to_pem_pkcs1()?,
        )?)
    }

    pub fn n(&self) -> Result<Vec<u8>> {
        Ok(self.private_key.rsa()?.n().to_vec())
    }

    pub fn e(&self) -> Result<Vec<u8>> {
        Ok(self.private_key.rsa()?.e().to_vec())
    }
}

impl PrivateKeyToJwk for RsaPrivateKey {
    #[allow(clippy::many_single_char_names)]
    fn private_key_to_jwk(&self) -> Result<Jwk> {
        let n = self.n()?;
        let e = self.e()?;
        let rsa = self.private_key.rsa()?;
        let d = rsa.d().to_vec();
        let p = rsa.p().map(|p| p.to_vec());
        let q = rsa.q().map(|q| q.to_vec());
        let dp = rsa.dmp1().map(|dp| dp.to_vec());
        let dq = rsa.dmq1().map(|dq| dq.to_vec());
        let qi = rsa.iqmp().map(|qi| qi.to_vec());
        fn encode(x: &[u8]) -> String {
            base64::encode_config(x, url_safe_trailing_bits())
        }
        Ok(Jwk {
            kty: "RSA".into(),
            alg: if self.verify_any {
                None
            } else {
                Some(self.algorithm.name().into())
            },
            use_: Some("sig".into()),
            n: Some(encode(&n)),
            e: Some(encode(&e)),
            d: Some(encode(&d)),
            p: p.map(|p| encode(&p)),
            q: q.map(|q| encode(&q)),
            dp: dp.map(|dp| encode(&dp)),
            dq: dq.map(|dq| encode(&dq)),
            qi: qi.map(|qi| encode(&qi)),
            ..Default::default()
        })
    }
}

impl PublicKeyToJwk for RsaPrivateKey {
    fn public_key_to_jwk(&self) -> Result<Jwk> {
        Ok(Jwk {
            kty: "RSA".into(),
            alg: if self.verify_any {
                None
            } else {
                Some(self.algorithm.name().into())
            },
            use_: Some("sig".into()),
            n: Some(base64::encode_config(self.n()?, url_safe_trailing_bits())),
            e: Some(base64::encode_config(self.e()?, url_safe_trailing_bits())),
            ..Jwk::default()
        })
    }
}

/// RSA Public Key.
#[derive(Debug)]
pub struct RsaPublicKey {
    public_key: PKey<Public>,
    /// If this is `None`, this key verifies signatures generated by ANY RSA
    /// algorithms. Otherwise it ONLY verifies signatures generated by this
    /// algorithm.
    pub algorithm: Option<RsaAlgorithm>,
}

impl RsaPublicKey {
    pub(crate) fn from_pkey(pkey: PKey<Public>, algorithm: Option<RsaAlgorithm>) -> Result<Self> {
        if pkey.id() != Id::RSA || pkey.bits() < 2048 {
            return Err(Error::UnsupportedOrInvalidKey);
        }
        Ok(Self {
            public_key: pkey,
            algorithm,
        })
    }

    /// Both `BEGIN PUBLIC KEY` and `BEGIN RSA PUBLIC KEY` are OK.
    pub fn from_pem(pem: &[u8], algorithm: Option<RsaAlgorithm>) -> Result<Self> {
        if std::str::from_utf8(pem).map_or(false, |pem| pem.contains("BEGIN RSA")) {
            let rsa = Rsa::public_key_from_pem_pkcs1(pem)?;
            Self::from_pkey(PKey::from_rsa(rsa)?, algorithm)
        } else {
            let pkey = PKey::public_key_from_pem(pem)?;
            Self::from_pkey(pkey, algorithm)
        }
    }

    pub fn from_components(n: &[u8], e: &[u8], algorithm: Option<RsaAlgorithm>) -> Result<Self> {
        let rsa = Rsa::from_public_components(BigNum::from_slice(n)?, BigNum::from_slice(e)?)?;
        Self::from_pkey(PKey::from_rsa(rsa)?, algorithm)
    }

    /// BEGIN PUBLIC KEY
    pub fn to_pem(&self) -> Result<String> {
        Ok(String::from_utf8(self.public_key.public_key_to_pem()?)?)
    }

    /// BEGIN RSA PUBLIC KEY
    pub fn to_pem_pkcs1(&self) -> Result<String> {
        Ok(String::from_utf8(
            self.public_key.rsa()?.public_key_to_pem_pkcs1()?,
        )?)
    }

    pub fn n(&self) -> Result<Vec<u8>> {
        Ok(self.public_key.rsa()?.n().to_vec())
    }

    pub fn e(&self) -> Result<Vec<u8>> {
        Ok(self.public_key.rsa()?.e().to_vec())
    }
}

impl PublicKeyToJwk for RsaPublicKey {
    fn public_key_to_jwk(&self) -> Result<Jwk> {
        Ok(Jwk {
            kty: "RSA".into(),
            alg: self.algorithm.map(|alg| alg.name().to_string()),
            use_: Some("sig".into()),
            n: Some(base64::encode_config(self.n()?, url_safe_trailing_bits())),
            e: Some(base64::encode_config(self.e()?, url_safe_trailing_bits())),
            ..Jwk::default()
        })
    }
}

impl SigningKey for RsaPrivateKey {
    fn sign(&self, v: &[u8]) -> Result<SmallVec<[u8; 64]>> {
        let mut signer = Signer::new(self.algorithm.digest(), self.private_key.as_ref())?;
        if self.algorithm.is_pss() {
            signer.set_rsa_padding(Padding::PKCS1_PSS)?;
            signer.set_rsa_pss_saltlen(RsaPssSaltlen::DIGEST_LENGTH)?;
        }

        signer.update(v)?;
        Ok(signer.sign_to_vec()?.into())
    }

    fn alg(&self) -> &'static str {
        self.algorithm.name()
    }
}

impl VerificationKey for RsaPrivateKey {
    fn verify(&self, v: &[u8], sig: &[u8], alg: &str) -> Result<()> {
        let alg = if self.verify_any {
            RsaAlgorithm::from_name(alg)?
        } else {
            if alg != self.algorithm.name() {
                return Err(Error::VerificationError);
            }
            self.algorithm
        };

        let mut verifier = Verifier::new(alg.digest(), self.private_key.as_ref())?;
        if alg.is_pss() {
            verifier.set_rsa_padding(Padding::PKCS1_PSS)?;
            verifier.set_rsa_pss_saltlen(RsaPssSaltlen::DIGEST_LENGTH)?;
        }
        if verifier.verify_oneshot(sig, v)? {
            Ok(())
        } else {
            Err(Error::VerificationError)
        }
    }
}

impl VerificationKey for RsaPublicKey {
    fn verify(&self, v: &[u8], sig: &[u8], alg: &str) -> Result<()> {
        let alg = if let Some(self_alg) = self.algorithm {
            if self_alg.name() != alg {
                return Err(Error::VerificationError);
            }
            self_alg
        } else {
            RsaAlgorithm::from_name(alg)?
        };

        let mut verifier = Verifier::new(alg.digest(), self.public_key.as_ref())?;
        if alg.is_pss() {
            verifier.set_rsa_padding(Padding::PKCS1_PSS)?;
            verifier.set_rsa_pss_saltlen(RsaPssSaltlen::DIGEST_LENGTH)?;
        }
        if verifier.verify_oneshot(sig, v)? {
            Ok(())
        } else {
            Err(Error::VerificationError)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ecdsa::{EcdsaAlgorithm, EcdsaPrivateKey},
        SomePrivateKey,
    };

    use super::*;

    #[test]
    fn conversion() -> Result<()> {
        let k = RsaPrivateKey::generate(2048, RsaAlgorithm::PS384)?;
        let pem = k.private_key_to_pem_pkcs8()?;
        RsaPrivateKey::from_pem(pem.as_bytes(), RsaAlgorithm::PS384)?;

        let es256key_pem =
            EcdsaPrivateKey::generate(EcdsaAlgorithm::ES256)?.private_key_to_pem_pkcs8()?;
        assert!(RsaPrivateKey::from_pem(es256key_pem.as_bytes(), RsaAlgorithm::PS384).is_err());

        let pk_pem = k.public_key_to_pem()?;
        let pk_pem_pkcs1 = k.public_key_to_pem_pkcs1()?;

        let pk = RsaPublicKey::from_pem(pk_pem.as_bytes(), None)?;
        let pk1 = RsaPublicKey::from_pem(pk_pem_pkcs1.as_bytes(), None)?;

        println!("pk: {:?}", pk);

        let pk_pem1 = pk1.to_pem()?;
        let pk_pem_pkcs1_1 = pk.to_pem_pkcs1()?;

        assert_eq!(pk_pem, pk_pem1);
        assert_eq!(pk_pem_pkcs1, pk_pem_pkcs1_1);

        assert_eq!(k.alg(), "PS384");

        if let SomePrivateKey::Rsa(k1) = k
            .private_key_to_jwk()?
            .to_signing_key(RsaAlgorithm::RS512)?
        {
            assert!(k.private_key.public_eq(k1.private_key.as_ref()));
        } else {
            panic!("expected rsa private key");
        }

        k.public_key_to_jwk()?.to_verification_key()?;
        pk.public_key_to_jwk()?;

        Ok(())
    }

    #[test]
    fn test_private_key_from_jwk_n_e_d_only() -> Result<()> {
        let k = RsaPrivateKey::generate(2048, RsaAlgorithm::PS256)?;
        let mut jwk = k.private_key_to_jwk()?;
        jwk.p = None;
        jwk.q = None;
        jwk.dp = None;
        jwk.dq = None;
        jwk.qi = None;
        let k1 = jwk.to_signing_key(RsaAlgorithm::RS256)?;
        let sig = k1.sign(b"msg")?;
        k.verify(b"msg", &sig, "PS256")?;
        k1.verify(b"msg", &sig, "PS256")?;
        let sig = k.sign(b"msg")?;
        k1.verify(b"msg", &sig, "PS256")?;
        Ok(())
    }

    #[test]
    fn sign_verify() -> Result<()> {
        for alg in [
            RsaAlgorithm::RS256,
            RsaAlgorithm::RS384,
            RsaAlgorithm::RS512,
            RsaAlgorithm::PS256,
            RsaAlgorithm::PS384,
            RsaAlgorithm::PS512,
        ] {
            let k = RsaPrivateKey::generate(2048, alg)?;
            let pk = RsaPublicKey::from_pem(k.public_key_to_pem()?.as_bytes(), None)?;
            let sig = k.sign(b"...")?;
            assert!(k.verify(b"...", &sig, alg.name()).is_ok());
            assert!(k.verify(b"...", &sig, "WRONG ALG").is_err());
            assert!(k.verify(b"....", &sig, alg.name()).is_err());
            assert!(pk.verify(b"...", &sig, alg.name()).is_ok());
            assert!(pk.verify(b"....", &sig, alg.name()).is_err());
        }
        Ok(())
    }
}

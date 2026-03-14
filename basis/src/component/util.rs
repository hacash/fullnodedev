

macro_rules! impl_pkg_common {
    ($class: ty, $pty: ident, $origin: ty) => {

        impl $class {

            pub fn create(objc: Box<dyn $pty>) -> Self {
                let data = objc.serialize();
                Self::new(objc, data)
            }

            pub fn set_origin(&mut self, orgi: $origin) {
                self.orgi = orgi;
            }

            pub fn origin(&self) -> $origin {
                self.orgi
            }

            pub fn seek(&self) -> usize {
                self.seek
            }

            pub fn size(&self) -> usize {
                self.size
            }

            pub fn data(&self) -> &[u8] {
                let sk = self.seek;
                &self.data.as_ref()[sk .. sk + self.size]
            }

            pub fn copy_data(&self) -> Vec<u8> {
                self.data().to_vec()
            }

        }

    };
}

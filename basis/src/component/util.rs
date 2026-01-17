

macro_rules! impl_pkg_common {
    ($class: ty, $pty: ident, $origin: ty) => {

        impl $class {

            pub fn apart(self) -> (Hash, Box<dyn $pty>, Vec<u8>) {
                (self.hash, 
                    self.objc, 
                    self.data[self.seek .. self.seek + self.size].to_vec()
                )
            }

            pub fn create(objc: Box<dyn $pty>) -> Self {
                let data = objc.serialize();
                Self::new(objc, data)
            }

            pub fn set_origin(&mut self, orgi: $origin) {
                self.orgi = orgi;
            }

            pub fn data(&self) -> &[u8] {
                &self.data.as_ref()[self.seek..self.size]
            }

            pub fn into(self) -> Box<dyn $pty> {
                self.objc
            }
        
        }

    };
}
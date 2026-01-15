

macro_rules! impl_pkg_common {
    ($class: ty, $pty: ident, $origin: ty) => {

        impl $class {

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
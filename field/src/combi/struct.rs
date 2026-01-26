

#[macro_export]
macro_rules! combi_struct {
    ($class:ident, $( $item:ident : $type:ty )+ ) => (
        

        #[derive(Default, Debug, Clone, PartialEq, Eq)]
        pub struct $class {
            $(
                pub $item: $type
            ),+
        }

        impl Parse for $class {
            fn parse_from(&mut self, buf: &mut &[u8]) -> Ret<usize> {
                let mut mv = 0;
                $( mv += self.$item.parse_from(buf)?; )+
                Ok(mv)
            }
        }

        impl Serialize for $class {
            fn serialize_to(&self, out: &mut Vec<u8>) {
                $( self.$item.serialize_to(out); )+
            }
            fn size(&self) -> usize {
                [ $( self.$item.size() ),+ ].iter().sum()
            }
        }

        impl_field_only_new!{$class}



    )
}


// test
combi_struct!{ Test73895763489564,
    aaa: Uint1
}









#[macro_export]
macro_rules! combi_struct_with_parse_serialize {
    ($class:ident, ( $this:ident, $buf:ident, $parse:expr, $serialize:expr, $size:expr ), $( $item:ident : $type:ty )+ ) => (

        #[derive(Default, Debug, Clone, PartialEq, Eq)]
        pub struct $class {
            $(
                pub $item: $type
            ),+
        }

        impl Parse for $class {
            fn parse(&mut $this, $buf: &[u8]) -> Ret<usize> {
                $parse
            }
        }

        impl Serialize for $class {
            fn serialize(&$this) -> Vec<u8> {
                $serialize
            }
            fn serialize_to(&$this, out: &mut Vec<u8>) {
                let v = $serialize;
                out.extend_from_slice(&v);
            }
            fn size(&$this) -> usize {
                $size
            }
        }

        impl_field_only_new!{$class}

    )
}


#[macro_export]
macro_rules! combi_struct_with_parse {
    ($class:ident, ( $this:ident, $buf:ident, $parse:expr ), $( $item:ident : $type:ty )+ ) => (

        combi_struct_with_parse_serialize!{$class, (
            $this, $buf, $parse,
            vec![ $( $this.$item.serialize() ),+ ].concat(),
            [ $( $this.$item.size() ),+ ].iter().sum()
        ), $( $item : $type )+ }

    )
}






#[macro_export]
macro_rules! combi_struct_field_more_than_condition {
    ($class:ident, { $( $item:ident : $type:ty )+ }, $mrn:ident, $mrv:ty, $cdn:ident, $cdv:expr ) => (

        #[derive(Default, Debug, Clone, PartialEq, Eq)]
        pub struct $class {
            $(
                pub $item: $type,
            )+
            pub $mrn: $mrv
        }

        impl Parse for $class {

            fn parse_from(&mut self, buf: &mut &[u8]) -> Ret<usize> {
                let mut mv = 0;
                $( mv += self.$item.parse_from(buf)?; )+
                if *self.$cdn > $cdv {
                    mv += self.$mrn.parse_from(buf)?;
                }
                Ok(mv)
            }

        }

        impl Serialize for $class {

            fn serialize_to(&self, out: &mut Vec<u8>) {
                $( self.$item.serialize_to(out); )+
                if *self.$cdn > $cdv {
                    self.$mrn.serialize_to(out);
                }
            }

            fn size(&self) -> usize {
                let mut sz = [ $( self.$item.size() ),+ ].iter().sum();
                if *self.$cdn > $cdv {
                    sz += self.$mrn.size();
                }
                sz
            }

        }

        impl_field_only_new!{$class}

    )
}



#[macro_export]
macro_rules! combi_dynvec {
    ($class:ident, $lenty:ty, $dynty:ident, $parseobjfunc: path, $json_createfn:path) => (


#[derive(Default, Clone)]
pub struct $class {
    count: $lenty,
    vlist: Vec<Box<dyn $dynty>>
}

impl std::fmt::Debug for $class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"[dyn list {}]", *self.count)
    }
}

impl PartialEq for $class {
    #[inline]
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl Eq for $class {}


impl Parse for $class {

    fn parse_from(&mut self, buf: &mut &[u8]) -> Ret<usize> {
        let mut seek = 0;
        let count = *self.count as usize;
        self.vlist = Vec::with_capacity(count);
        for _ in 0..count {
            let(obj, mvsk) = $parseobjfunc(*buf) ?;
            *buf = &(*buf)[mvsk..];
            seek += mvsk;
            self.vlist.push(obj);
        }
        Ok(seek)
    }
}

impl Serialize for $class {
    
    fn serialize_to(&self, out: &mut Vec<u8>) {
        for v in &self.vlist {
            v.serialize_to(out);
        }
    }

    fn size(&self) -> usize {
        let mut sznum = 0;
        for v in &self.vlist {
            sznum += v.size();
        }
        sznum
    }

}

impl_field_only_new!{$class}

impl ToJSON for $class {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        let mut res = String::from("[");
        for i in 0..self.vlist.len() {
            if i > 0 { res.push(','); }
            res.push_str(&self.vlist[i].as_ref().to_json_fmt(fmt));
        }
        res.push(']');
        res
    }
}

impl FromJSON for $class {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let (list, _) = $crate::json_decode_array(json)?;
        let mut vlist: Vec<Box<dyn $dynty>> = Vec::with_capacity(list.len());
        for item_json in list {
            if let Some(item) = $json_createfn(&item_json)? {
                vlist.push(item);
            } else {
                return errf!("dynamic object JSON decode error from: {}", item_json);
            }
        }
        self.vlist = vlist;
        self.count = <$lenty>::from_usize(self.vlist.len())?;
        Ok(())
    }
}

impl $class {

    pub fn replace(&mut self, i: usize, v: Box<dyn $dynty>) -> Rerr {
        let tl = self.length() as usize;
        if i >= tl {
            return errf!("list index overflow")
        }
        self.vlist[i] = v;
        Ok(())
    }

	pub fn push(&mut self, v: Box<dyn $dynty>) -> Rerr {
        if *self.count >= <$lenty>::MAX {
            return errf!("list length overflow");
        }
		self.count += 1u8;
        self.vlist.push(v);
        Ok(())
	}

	pub fn pop(&mut self) -> Option<Box<dyn $dynty>> {
        let n = *self.count;
        match n {
            0 => None,
            _ => {
                self.count -= 1u8;
                self.vlist.pop()
            }
        }
	}

	pub fn length(&self) -> usize {
		*self.count as usize
	}

	pub fn count(&self) -> &$lenty {
		&self.count
	}

	pub fn set_count(&mut self, c: $lenty) {
		self.count = c;
	}

    pub fn list(&self) -> &Vec<Box<dyn $dynty>> {
        &self.vlist
    }


}



    )
}


/********************* test **********************/ 
pub trait Test78756388732645 : Field + DynClone {
    fn getv(&self) -> u8 { 0 }
}
clone_trait_object!{Test78756388732645}

fn test_create_838464857639363(_a:&[u8])->Ret<(Box<dyn Test78756388732645>, usize)>{errf!("")}
fn test_json_create_838464857639363(_b:&str)->Ret<Option<Box<dyn Test78756388732645>>>{errf!("")}
combi_dynvec!{ Test294635492624,
    Uint1, Test78756388732645, test_create_838464857639363, test_json_create_838464857639363
}




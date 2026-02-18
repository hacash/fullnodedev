
macro_rules! define_tex_cell_create { ($f: ident, $( $ty: ty )+) => {
     

fn tex_cell_create(buf: &[u8])->Ret<(Box<dyn TexCell>, usize)>{
    let (cid, _) = Uint1::create(buf)?;
    Ok(match cid.uint() {
        $(
        <$ty>::CID => {
            let (obj, sz) = <$ty>::create(buf)?;
            (Box::new(obj), sz)
        }
        )+
        i => return errf!("cannot find tex cell id '{}'", i)
    })
}

fn tex_cell_try_json_decode(kind: u16, json: &str) -> Ret<Option<Box<dyn TexCell>>> {
    match kind {
        $(
        i if i == <$ty>::CID as u16 => {
            let mut obj = <$ty>::default();
            obj.from_json(json)?;
            Ok(Some(Box::new(obj)))
        }
        )+
        _ => Ok(None)
    }
}

fn tex_cell_json_decode(json: &str) -> Ret<Option<Box<dyn TexCell>>> {
    let obj = json_decode_object(json)?;
    let cellid_str = obj.get("cellid")
        .ok_or_else(|| "tex cell object JSON must have 'cellid'".to_string())?;
    let cellid = cellid_str.parse::<u16>()
        .map_err(|_| format!("invalid tex cell id: {}", cellid_str))?;
    tex_cell_try_json_decode(cellid, json)
}

   
}}




define_tex_cell_create!{ tex_cell_create, 

    CellTrsZhuPay      // 1
    CellTrsZhuGet      // 2
    CellTrsSatPay      // 3
    CellTrsSatGet      // 4
    CellTrsDiaPay      // 5
    CellTrsDiaGet      // 6
    CellTrsAssetPay    // 7
    CellTrsAssetGet    // 8 
    
    CellCondZhuAtMost   // 11
    CellCondZhuAtLeast  // 12
    CellCondZhuEq    // 13
    CellCondSatAtMost   // 14
    CellCondSatAtLeast  // 15
    CellCondSatEq    // 16
    CellCondDiaAtMost   // 17
    CellCondDiaAtLeast  // 18
    CellCondDiaEq    // 19
    CellCondAssetAtMost // 20
    CellCondAssetAtLeast // 21
    CellCondAssetEq  // 22
    CellCondHeightAtMost  // 23
    CellCondHeightAtLeast // 24
    

}



combi_dynlist!{ DnyTexCellW1, Uint1, TexCell, tex_cell_create, tex_cell_json_decode}



impl CellExec for DnyTexCellW1 {
    fn execute(&self, ctx: &mut dyn Context, main: &Address) -> Rerr {        
        for cell in self.as_list() {
            cell.execute(ctx, main)?;
        }
        Ok(())
    }
}

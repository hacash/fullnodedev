
#[allow(unused)]
fn main() {

    protocol::setup::block_hasher( x16rs::block_hash );

    #[cfg(feature = "tex")]
    protocol::setup::action_register( protocol::tex::try_create );


    println!("Hello, Fullnode")

}
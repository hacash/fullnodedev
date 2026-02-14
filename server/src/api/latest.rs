

api_querys_define!{ Q4376,
    __nnn_, Option<bool>, None,
}

async fn latest(State(ctx): State<ApiCtx>, _q: Query<Q4376>) -> impl IntoResponse {
    let lasthei = ctx.engine.latest_block().height().uint();
    let data = jsondata!{
        "height", lasthei,
    };
    api_data(data)

}

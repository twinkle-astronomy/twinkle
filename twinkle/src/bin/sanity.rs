use actix::prelude::*;
use futures_util::stream::once;

#[derive(Message)]
#[rtype(result = "()")]
struct Ping;

struct MyActor;

impl StreamHandler<Ping> for MyActor {
    fn handle(&mut self, item: Ping, ctx: &mut Context<MyActor>) {
        println!("PING");
        System::current().stop()
    }

    fn finished(&mut self, ctx: &mut Self::Context) {
        println!("finished");
    }
}

impl Actor for MyActor {
   type Context = Context<Self>;

   fn started(&mut self, ctx: &mut Context<Self>) {
       Self::add_stream(once(async { Ping }), ctx);
   }
}

fn main() {
    let mut sys = System::new();
    let addr = sys.block_on(async { MyActor.start() });
    sys.run();
}

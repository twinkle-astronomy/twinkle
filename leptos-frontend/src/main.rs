
use leptos::{ prelude::*, task::spawn_local};
use leptos::mount::mount_to_body;
use tokio_stream::StreamExt;
use leptos::logging::error;

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).expect("error initializing logger");
    tracing_wasm::set_as_global_default();
    mount_to_body(|| view! { <App/> })
}

#[component]
fn App() -> impl IntoView {
    view! {
        <TwinkleSocket/>
    }
}


#[component]
fn TwinkleSocket() -> impl IntoView {
    let (client, client_update) = signal(None::<indi::client::Client>);
    let (device_list, device_list_update) = signal(None);

    let open_connection = move |_| {

        spawn_local(async move {
            let connection = tokio_tungstenite_wasm::connect("ws://localhost:4000/indi").await.unwrap();
            let client = indi::client::new(connection, None,None);
            if let Ok(client) = client {

                let mut sub = client.get_devices().subscribe().await;
                spawn_local(async move {
                    loop {
                        let devices = match sub.next().await {
                            Some(Ok(devices)) => devices,
                            Some(Err(e)) => {
                                error!("Got error: {:?}", e);
                                break;
                            }
                            None => break,
                        };
                        device_list_update.update(move |device_list_update| {
                            device_list_update.replace(devices);
                        })
                    }
                    device_list_update.update(|device_list_update| {
                        device_list_update.take();
                    });
                    client_update.update(|client_update| {
                        client_update.take();
                    })
                });
                
                client_update.update(|client_update| {
                    client_update.replace(client);
                });
            }
        });

    };
    let connected = move || {
        let ready_state = client.with(|client| client.is_some());
        match ready_state {
            true => "Connected",
            false => "Disconnected"
        }
    };

    let is_connected = move || {
        client.with(|client| client.is_some())
    };
        
    let close_connection = {
        move |_| {
            client_update.update(|client| {
                if let Some(client) = client {
                    client.shutdown();
                }
                client.take();
            });
        }
    };
    view! {
        <div>
            <p>"status: " {connected}</p>
                
            <button on:click=close_connection hidden= move|| !is_connected() >"Close"</button>
            <button on:click=open_connection  hidden= move|| is_connected()>"Open"</button>
    
            {
                move || if let Some(device_list) = device_list.get() {
                    view! {
                        <For
                            each=move ||{
                                device_list.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>()
                            }
                            key=|device| device.0.clone()
                            let:child
                        >
                            <p>{child.0.clone()}</p>
                        </For>
            
                    }.into_any()
                } else {
                    view! { <p> None </p> }.into_any()
                }
            }
        </div>
    }
    
}


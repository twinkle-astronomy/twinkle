use indi::{serialization::GetProperties, INDI_PROTOCOL_VERSION};
use leptos::*;
use leptos_use::{core::ConnectionReadyState, use_websocket, UseWebSocketReturn};
use codee::string::FromToStringCodec;

fn main() {
    leptos::mount_to_body(|| view! { <App/> })
}

#[component]
fn App() -> impl IntoView {
    view! {
        <TwinkleSocket/>
    }
}

#[component]
fn ProgressBar(
    #[prop(default=100)]
    max: usize,
    progress: impl Fn() -> i32 + 'static
) -> impl IntoView {
    view! {
        <progress
            max=max
            value=progress
        />
    }
}

#[component]
fn TwinkleSocket() -> impl IntoView {
    let UseWebSocketReturn {
        ready_state,
        message,
        send,
        open,
        close,
        ..
    } = use_websocket::<String, String, FromToStringCodec>("ws://localhost:4000/");
    
    let send_message = move |_| {
        send(
            &quick_xml::se::to_string(&indi::serialization::Command::GetProperties(GetProperties {
                version: INDI_PROTOCOL_VERSION.to_string(),
                device: None,
                name: None,
            }
            )).unwrap()
        );
    };
    
    let status = move || ready_state.get().to_string();
    
    let connected = move || ready_state.get() == ConnectionReadyState::Open;
    
    let open_connection = move |_| {
        open();
    };
    
    let close_connection = move |_| {
        close();
    };
    
    view! {
        <div>
            <p>"status: " {status}</p>
    
            <button on:click=send_message disabled=move || !connected()>"Send"</button>
            <button on:click=open_connection disabled=connected>"Open"</button>
            <button on:click=close_connection disabled=move || !connected()>"Close"</button>
    
            <p>"Receive message: " {move || format!("{:?}", message.get())}</p>
        </div>
    }
    
}

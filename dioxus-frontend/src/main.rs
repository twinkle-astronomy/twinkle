use dioxus::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut count = use_signal(|| 0);

    rsx! {
        div {
            style: "text-align: center; padding: 20px; font-family: Arial, sans-serif;",
            h1 { "Hello from Dioxus!" }
            h2 { "Counter: {count}" }
            
            div {
                style: "margin: 20px;",
                button {
                    style: "margin: 10px; padding: 10px 20px; font-size: 16px; cursor: pointer;",
                    onclick: move |_| count += 1,
                    "Increment"
                }
                button {
                    style: "margin: 10px; padding: 10px 20px; font-size: 16px; cursor: pointer;",
                    onclick: move |_| count -= 1,
                    "Decrement"
                }
                button {
                    style: "margin: 10px; padding: 10px 20px; font-size: 16px; cursor: pointer;",
                    onclick: move |_| count.set(0),
                    "Reset"
                }
            }
            
            p {
                style: "margin-top: 30px; color: #666;",
                "This is a simple Dioxus web application with a counter."
            }
        }
    }
}
use dioxus::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut count = use_signal(|| 0);

    rsx! {
        div {
            class: "min-h-screen bg-gray-100 dark:bg-gray-900 flex flex-col items-center justify-center p-8",
            div {
                class: "bg-white dark:bg-gray-800 rounded-lg shadow-lg p-8 w-full max-w-md",
                h1 { 
                    class: "text-4xl font-bold text-center text-gray-800 dark:text-white mb-2",
                    "Hello from Dioxus!" 
                }
                h2 { 
                    class: "text-2xl font-semibold text-center text-gray-600 dark:text-gray-300 mb-8",
                    "Counter: {count}" 
                }
                
                div {
                    class: "flex flex-col sm:flex-row gap-4 justify-center mb-8",
                    button {
                        class: "bg-blue-500 hover:bg-blue-600 text-white font-semibold py-2 px-6 rounded-lg transition-colors duration-200 cursor-pointer",
                        onclick: move |_| count += 1,
                        "Increment"
                    }
                    button {
                        class: "bg-red-500 hover:bg-red-600 text-white font-semibold py-2 px-6 rounded-lg transition-colors duration-200 cursor-pointer",
                        onclick: move |_| count -= 1,
                        "Decrement"
                    }
                    button {
                        class: "bg-gray-500 hover:bg-gray-600 text-white font-semibold py-2 px-6 rounded-lg transition-colors duration-200 cursor-pointer",
                        onclick: move |_| count.set(0),
                        "Reset"
                    }
                }
                
                p {
                    class: "text-center text-gray-600 dark:text-gray-400 text-sm",
                    "This is a simple Dioxus web application with a counter."
                }
            }
        }
    }
}
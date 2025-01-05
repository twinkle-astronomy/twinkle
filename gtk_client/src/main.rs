use std::env;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use gtk::glib::clone;
use gtk::{prelude::*, ListBox, TextBuffer, TextView};
use gtk::{glib, Application, ApplicationWindow, Button};
// use tokio::net::TcpStream;
// use tokio::runtime::Runtime;
use indi::{client::{AsyncClientConnection, AsyncReadConnection, AsyncWriteConnection}, serialization::GetProperties, INDI_PROTOCOL_VERSION};
// use indi::tokio::net::TcpStream;
use indi::tokio::runtime::Runtime;

use indi::client::websocket::tokio_tungstenite::connect_async;

const APP_ID: &str = "org.twinkle-astronomy.twinkle";

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        Runtime::new().expect("Setting up tokio runtime needs to succeed.")
    })
}

fn main() -> glib::ExitCode {
    // Create a new application
    let app = Application::builder().application_id(APP_ID).build();

    // Connect to "activate" signal of `app`
    app.connect_activate(build_ui);

    // Run the application
    app.run()
}

fn build_ui(app: &Application) {
    let main_area = ListBox::new();

    // Create a button with label and margins
    let button = Button::builder()
        .label("Press me!")
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    main_area.append(&button);

    let text_buffer = TextBuffer::builder().text("hello world").build();
    let text_view = TextView::builder().buffer(&text_buffer).editable(false).build();
    
    // Create a ScrolledWindow and add the TextView to it
    let scrolled_window = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&text_view)
        .margin_start(12)
        .margin_end(12)
        .margin_bottom(12)
        .build();

    // Set minimum size for the scrolled window
    scrolled_window.set_min_content_height(200);
    scrolled_window.set_min_content_width(300);

    // Add the ScrolledWindow to main_area instead of the TextView directly
    main_area.append(&scrolled_window);

    let (sender, mut receiver) = indi::tokio::sync::broadcast::channel(100);
    // Connect to "clicked" signal of `button`
    button.connect_clicked(move |_| {
        // The main loop executes the asynchronous block
        runtime().spawn(clone!(
            #[strong]
            sender,
            async move {
                let (websocket, _) = connect_async("ws://localhost:4000/indi".to_string()).await.unwrap();

                let (mut write, mut read) = websocket.to_indi();
            

                let reader = indi::tokio::spawn(async move {
                    loop {
                        match read.read().await {
                            Some(Ok(msg)) => {
                                sender.send(Arc::new(msg)).expect("Sending");
                            },
                            Some(Err(e)) => {
                                dbg!(e);
                            }
                            None => {
                                break;
                            }
                        }
                    }
            
                });
            
                let cmd = indi::serialization::Command::GetProperties(GetProperties {
                    version: INDI_PROTOCOL_VERSION.to_string(),
                    device: None,
                    name: None,
                });
                write.write(cmd).await.expect("Sending command");
                
            
                reader.await.ok();
            }
        ));
    });

    // The main loop executes the asynchronous block
    glib::spawn_future_local(clone!(
        #[strong]
        text_buffer,
        async move {
        while let Ok(response) = receiver.recv().await {
            println!("Status: {:?}", response);
            // glib::MainContext::default().spawn_local(clone!(
            //     #[strong]
            //     text_buffer,
            //     move || {
                    let mut end_iter = text_buffer.end_iter();
                    text_buffer.insert(&mut end_iter, format!("\n{:?}", response).as_str());
                // }
            // ));
        }
    }));

    // Create a window
    let window = ApplicationWindow::builder()
        .application(app)
        .title("My GTK App")
        .child(&main_area)
        .build();

    // Present window
    window.present();
}
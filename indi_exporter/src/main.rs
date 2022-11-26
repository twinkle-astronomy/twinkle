use indi;
use std::env;

use prometheus_exporter::{self, prometheus::*};

fn main() {
    let args: Vec<String> = env::args().collect();
    let addr = match args.len() {
        2 => args[1].as_str(),
        _ => "localhost:7624",
    };

    let mut connection = indi::Connection::new(addr).unwrap();
    connection
        .send(&indi::GetProperties {
            version: indi::INDI_PROTOCOL_VERSION.to_string(),
            device: None,
            name: None,
        })
        .unwrap();

    let mut client = indi::Client::new();

    let binding = "0.0.0.0:9186".parse().unwrap();
    prometheus_exporter::start(binding).unwrap();
    let gauge = register_gauge_vec!(
        opts!("indi_device_parameter_number", "help"),
        &[
            "device_name",
            "param_name",
            "param_label",
            "value_name",
            "value_label"
        ]
    )
    .unwrap();
    let states = register_gauge_vec!(
        opts!("indi_device_parameter_state", "help"),
        &["device_name", "param_name", "param_label", "state"]
    )
    .unwrap();

    for command in connection.command_iter().unwrap() {
        match command {
            Ok(command) => {
                println!("Command: {:?}", command);
                let device_name = command.device_name().unwrap().clone();
                match client.update(command) {
                    Err(e) => {
                        println!("error: {:?}", e)
                    }
                    Ok(Some(param_enum)) => {
                        state_metric(&states, &device_name, &param_enum);
                        match param_enum {
                            indi::Parameter::NumberVector(param) => {
                                for (value_name, value) in &param.values {
                                    gauge
                                        .with_label_values(&[
                                            device_name.as_str(),
                                            param.name.as_str(),
                                            param
                                                .label
                                                .as_ref()
                                                .unwrap_or(&"".to_string())
                                                .as_str(),
                                            value_name.as_str(),
                                            value
                                                .label
                                                .as_ref()
                                                .unwrap_or(&"".to_string())
                                                .as_str(),
                                        ])
                                        .set(value.value);
                                }
                            }
                            indi::Parameter::SwitchVector(param) => {
                                for (value_name, value) in &param.values {
                                    let v = if value.value == indi::SwitchState::On {
                                        1.0
                                    } else {
                                        0.0
                                    };
                                    gauge
                                        .with_label_values(&[
                                            device_name.as_str(),
                                            param.name.as_str(),
                                            param
                                                .label
                                                .as_ref()
                                                .unwrap_or(&"".to_string())
                                                .as_str(),
                                            value_name.as_str(),
                                            value
                                                .label
                                                .as_ref()
                                                .unwrap_or(&"".to_string())
                                                .as_str(),
                                        ])
                                        .set(v);
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(None) => {}
                }
            }
            Err(e) => match e {
                e => println!("error: {:?}", e),
            },
        }
    }
}

fn state_metric(states: &GaugeVec, device_name: &String, param: &indi::Parameter) {
    for state in &[
        indi::PropertyState::Idle,
        indi::PropertyState::Ok,
        indi::PropertyState::Busy,
        indi::PropertyState::Alert,
    ] {
        states
            .with_label_values(&[
                device_name.as_str(),
                param.get_name().as_str(),
                param
                    .get_label()
                    .as_ref()
                    .unwrap_or(&"".to_string())
                    .as_str(),
                format!("{:?}", state).as_str(),
            ])
            .set(if state == param.get_state() { 1.0 } else { 0.0 });
    }
}

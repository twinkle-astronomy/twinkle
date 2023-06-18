use indi::client::{ClientConnection, DeviceStore};
use prometheus::core::GenericGaugeVec;
use std::{collections::HashMap, env, net::TcpStream};

use prometheus_exporter::{
    self,
    prometheus::{core::AtomicF64, *},
};

struct Metrics {
    devices: indi::client::MemoryDeviceStore,
    gauge: GenericGaugeVec<AtomicF64>,
    states: GenericGaugeVec<AtomicF64>,
}

impl Metrics {
    fn new(gauge: GenericGaugeVec<AtomicF64>, states: GenericGaugeVec<AtomicF64>) -> Self {
        Metrics {
            devices: HashMap::new(),
            states,
            gauge,
        }
    }

    fn handle_command(&mut self, command: indi::serialization::Command) {
        let device_name = command.device_name().unwrap().clone();
        let update_result = self.devices.update(command, |update| {
            match update {
                indi::client::device::ParamUpdateResult::NoUpdate => {}
                indi::client::device::ParamUpdateResult::ExistingParam(param_enum) => {
                    state_metric(&self.states, &device_name, &param_enum);
                    match param_enum.as_ref() {
                        indi::Parameter::NumberVector(param) => {
                            for (value_name, value) in &param.values {
                                self.gauge
                                    .with_label_values(&[
                                        device_name.as_str(),
                                        param.name.as_str(),
                                        param.label.as_ref().unwrap_or(&"".to_string()).as_str(),
                                        value_name.as_str(),
                                        value.label.as_ref().unwrap_or(&"".to_string()).as_str(),
                                    ])
                                    .set(value.value.into());
                            }
                        }
                        indi::Parameter::SwitchVector(param) => {
                            for (value_name, value) in &param.values {
                                let v = if value.value == indi::SwitchState::On {
                                    1.0
                                } else {
                                    0.0
                                };
                                self.gauge
                                    .with_label_values(&[
                                        device_name.as_str(),
                                        param.name.as_str(),
                                        param.label.as_ref().unwrap_or(&"".to_string()).as_str(),
                                        value_name.as_str(),
                                        value.label.as_ref().unwrap_or(&"".to_string()).as_str(),
                                    ])
                                    .set(v);
                            }
                        }
                        _ => {}
                    }
                }
                indi::client::device::ParamUpdateResult::DeletedParams(deleted_params) => {
                    for param in deleted_params {
                        let param = param.lock().unwrap();
                        remove_state_metric(&self.states, &device_name, &param);
                        match param.as_ref() {
                            indi::Parameter::NumberVector(param) => {
                                for (value_name, value) in &param.values {
                                    self.gauge
                                        .remove_label_values(&[
                                            // .with_label_values(&[
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
                                        .unwrap();
                                }
                            }
                            indi::Parameter::SwitchVector(param) => {
                                for (value_name, value) in &param.values {
                                    self.gauge
                                        .remove_label_values(&[
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
                                        .unwrap();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        });
        if let Err(e) = update_result {
            dbg!(e);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let addr = match args.len() {
        2 => args[1].as_str(),
        _ => "localhost:7624",
    };

    let connection = TcpStream::connect(addr).unwrap();
    connection
        .write(&indi::serialization::GetProperties {
            version: indi::INDI_PROTOCOL_VERSION.to_string(),
            device: None,
            name: None,
        })
        .unwrap();

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
    let mut metrics = Metrics::new(gauge, states);

    for command in connection.iter().unwrap() {
        match command {
            Ok(command) => {
                metrics.handle_command(command);
            }
            Err(e) => {
                println!("{:?}", e);

                if let indi::serialization::DeError::IoError(_) = e {
                    return;
                }
            }
        }
    }
}

fn remove_state_metric(states: &GaugeVec, device_name: &String, param: &indi::Parameter) {
    for state in &[
        indi::PropertyState::Idle,
        indi::PropertyState::Ok,
        indi::PropertyState::Busy,
        indi::PropertyState::Alert,
    ] {
        states
            .remove_label_values(&[
                device_name.as_str(),
                param.get_name().as_str(),
                param
                    .get_label()
                    .as_ref()
                    .unwrap_or(&"".to_string())
                    .as_str(),
                format!("{:?}", state).as_str(),
            ])
            .unwrap();
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

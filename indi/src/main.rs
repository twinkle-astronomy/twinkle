use indi;

fn main() {
    let mut client = indi::Client::new("localhost:7624").unwrap();
    client.query_devices();
    client.listen_for_updates();
    // let xml = r#"
    // <tag1 att1 = "test">
    //     <tag2><!--Test comment-->Test</tag2>
    //     <tag2>
    //         Test 2
    //     </tag2>
    // </tag1>
    // <tag3 att1 = "test">
    //     <tag4><!--Test comment-->Test</tag4>
    //     <tag5>
    //         Test 2
    //     </tag5>
    // </tag3>
    //             "#;

    // let mut reader = Reader::from_str(xml);
    // reader.trim_text(true);

    // let mut indent = 0;
    // let mut buf = Vec::new();

    // // The `Reader` does not implement `Iterator` because it outputs borrowed data (`Cow`s)
    // loop {
    //     match reader.read_event(&mut buf) {
    //     // for triggering namespaced events, use this instead:
    //     // match reader.read_namespaced_event(&mut buf) {
    //         Ok(Event::Start(e)) => {
    //             println!("{}Event::Start: {:?}", " ".repeat(indent), e);
    //             indent += 1;
    //         // for namespaced:
    //         // Ok((ref namespace_value, Event::Start(ref e)))
    //             // match e.name() {
    //             //     b"tag1" => println!("attributes values: {:?}",
    //             //                         e.attributes().map(|a| a.unwrap().value)
    //             //                         .collect::<Vec<_>>()),
    //             //     b"tag2" => count += 1,
    //             //     _ => (),
    //             // }
    //         },
    //         // unescape and decode the text event using the reader encoding
    //         Ok(Event::Text(ref e)) => {
    //             println!("{}Text: {:?}", " ".repeat(indent), e);
    //         },
    //         Ok(Event::End(ref e)) => {
    //             indent -= 1;
    //             println!("{}Event::End: {:?}", " ".repeat(indent), e);
    //         }
    //         Ok(Event::Eof) => break, // exits the loop when reaching end of file
    //         Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
    //         _ => (), // There are several other `Event`s we do not consider here
    //     }

    //     // if we don't keep a borrow elsewhere, we can clear the buffer to keep memory usage low
    //     buf.clear();
    // }
}

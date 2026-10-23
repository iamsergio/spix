// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The XML-RPC of the Spix protocol: the values its calls carry and the codec of a call and of an
//! answer. An untyped `<value>text</value>` is a string, which is also how Spix answers the calls
//! that return nothing.

use roxmltree::Document;

/// An XML-RPC value: the return types of the methods, and the types the calls send.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i32),
    Double(f64),
    String(String),
    Array(Vec<Value>),
}

/// A decoded `methodCall`.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub method: String,
    pub params: Vec<Value>,
}

/// Why a request can't be answered as asked. Each of these is answered with a fault.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("malformed XML-RPC request")]
    Xml(#[from] roxmltree::Error),
    #[error("malformed XML-RPC request: {0}")]
    Malformed(String),
    /// A method or a parameter type outside the protocol the tests speak.
    #[error("unsupported")]
    Unsupported,
}

/// Reads one `methodCall`. Only strings and ints arrive: what the Spix calls send.
///
/// # Errors
///
/// If the request isn't a well-formed `methodCall` with parameters of those two types.
pub fn decode_call(xml: &str) -> Result<Call, Error> {
    let document = Document::parse(xml)?;
    let root = document.root_element();
    if !root.has_tag_name("methodCall") {
        return Err(malformed(&root));
    }
    let mut method = None;
    let mut params = Vec::new();
    for child in root.children().filter(roxmltree::Node::is_element) {
        if child.has_tag_name("methodName") {
            method = Some(child.text().unwrap_or_default().to_owned());
        } else if child.has_tag_name("params") {
            for param in child.children().filter(roxmltree::Node::is_element) {
                if !param.has_tag_name("param") {
                    return Err(malformed(&param));
                }
                params.push(decode_value(&only_child(&param)?)?);
            }
        } else {
            return Err(malformed(&child));
        }
    }
    let method = method.ok_or_else(|| Error::Malformed("no methodName".to_owned()))?;
    Ok(Call { method, params })
}

/// The `methodResponse` for a return value. Spix answers the calls that return nothing with an
/// empty `<value>`, which the client reads as the empty string.
#[must_use]
pub fn encode_response(value: &Value) -> String {
    let mut xml =
        String::from(r#"<?xml version="1.0" encoding="utf-8"?><methodResponse><params><param>"#);
    encode_value(&mut xml, value);
    xml.push_str("</param></params></methodResponse>");
    xml
}

/// The `methodResponse` for a fault: a `faultCode` and a `faultString`.
#[must_use]
pub fn encode_fault(code: i32, message: &str) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="utf-8"?><methodResponse><fault><value><struct><member><name>faultCode</name>"#,
    );
    encode_value(&mut xml, &Value::Int(code));
    xml.push_str("</member><member><name>faultString</name>");
    encode_value(&mut xml, &Value::String(message.to_owned()));
    xml.push_str("</member></struct></value></fault></methodResponse>");
    xml
}

fn decode_value(node: &roxmltree::Node) -> Result<Value, Error> {
    if !node.has_tag_name("value") {
        return Err(malformed(node));
    }
    let Some(typed) = node.children().find(roxmltree::Node::is_element) else {
        return Ok(Value::String(node.text().unwrap_or_default().to_owned()));
    };
    let text = typed.text().unwrap_or_default();
    match typed.tag_name().name() {
        "string" => Ok(Value::String(text.to_owned())),
        "int" | "i4" => Ok(Value::Int(
            text.trim().parse().map_err(|_| malformed(&typed))?,
        )),
        // Nothing in the protocol sends the other XML-RPC types.
        _ => Err(Error::Unsupported),
    }
}

fn encode_value(xml: &mut String, value: &Value) {
    xml.push_str("<value>");
    match value {
        Value::Bool(what) => {
            let _ = std::fmt::Write::write_fmt(
                xml,
                format_args!("<boolean>{}</boolean>", u8::from(*what)),
            );
        }
        Value::Int(what) => {
            let _ = std::fmt::Write::write_fmt(xml, format_args!("<int>{what}</int>"));
        }
        Value::Double(what) => {
            let _ = std::fmt::Write::write_fmt(xml, format_args!("<double>{what}</double>"));
        }
        Value::String(what) => escape_into(xml, what),
        Value::Array(values) => {
            xml.push_str("<array><data>");
            for value in values {
                encode_value(xml, value);
            }
            xml.push_str("</data></array>");
        }
    }
    xml.push_str("</value>");
}

/// Writes `text` as the value of an untyped `<value>`, the way Spix writes its strings.
fn escape_into(xml: &mut String, text: &str) {
    for character in text.chars() {
        match character {
            '&' => xml.push_str("&amp;"),
            '<' => xml.push_str("&lt;"),
            '>' => xml.push_str("&gt;"),
            _ => xml.push(character),
        }
    }
}

fn malformed(node: &roxmltree::Node) -> Error {
    Error::Malformed(format!("unexpected <{}>", node.tag_name().name()))
}

fn only_child<'a, 'input>(
    node: &roxmltree::Node<'a, 'input>,
) -> Result<roxmltree::Node<'a, 'input>, Error> {
    let mut children = node.children().filter(roxmltree::Node::is_element);
    match (children.next(), children.next()) {
        (Some(child), None) => Ok(child),
        _ => Err(malformed(node)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // INFRA-004
    #[test]
    fn decodes_the_calls_the_tests_send() {
        let call = decode_call(
            r#"<?xml version="1.0"?><methodCall><methodName>enterKey</methodName><params><param><value><string>mainWindow/a</string></value></param><param><value><int>16777220</int></value></param></params></methodCall>"#,
        )
        .unwrap();
        assert_eq!(
            call,
            Call {
                method: "enterKey".to_owned(),
                params: vec![
                    Value::String("mainWindow/a".to_owned()),
                    Value::Int(16_777_220)
                ]
            }
        );
    }

    // INFRA-004
    #[test]
    fn decodes_the_typed_forms_and_an_untyped_string() {
        let decode = |param: &str| {
            let xml = format!(
                r#"<?xml version="1.0"?><methodCall><methodName>m</methodName><params><param><value>{param}</value></param></params></methodCall>"#
            );
            decode_call(&xml).unwrap().params.remove(0)
        };
        assert_eq!(
            decode("hello &amp; bye"),
            Value::String("hello & bye".to_owned())
        );
        assert_eq!(decode("<i4>-7</i4>"), Value::Int(-7));
        assert_eq!(decode("<string>x</string>"), Value::String("x".to_owned()));
    }

    // INFRA-004
    #[test]
    fn rejects_what_it_cannot_speak() {
        for xml in [
            "not xml",
            "<other/>",
            r#"<?xml version="1.0"?><methodCall><params><param><value><string>a</string></value></param></params></methodCall>"#,
            r#"<?xml version="1.0"?><methodCall><methodName>m</methodName><params><param><value><boolean>1</boolean></value></param></params></methodCall>"#,
        ] {
            assert!(decode_call(xml).is_err(), "accepted {xml}");
        }
    }

    // INFRA-004
    #[test]
    fn encodes_the_answer_shapes_spix_answers_with() {
        assert_eq!(
            encode_response(&Value::String(String::new())),
            r#"<?xml version="1.0" encoding="utf-8"?><methodResponse><params><param><value></value></param></params></methodResponse>"#
        );
        assert_eq!(
            encode_response(&Value::String("a <b> & ü".to_owned())),
            r#"<?xml version="1.0" encoding="utf-8"?><methodResponse><params><param><value>a &lt;b&gt; &amp; ü</value></param></params></methodResponse>"#
        );
        assert_eq!(
            encode_response(&Value::Bool(true)),
            r#"<?xml version="1.0" encoding="utf-8"?><methodResponse><params><param><value><boolean>1</boolean></value></param></params></methodResponse>"#
        );
        assert_eq!(
            encode_response(&Value::Array(vec![Value::Double(640.5)])),
            r#"<?xml version="1.0" encoding="utf-8"?><methodResponse><params><param><value><array><data><value><double>640.5</double></value></data></array></value></param></params></methodResponse>"#
        );
    }

    // INFRA-004
    #[test]
    fn encodes_a_fault_the_client_reads() {
        let xml = encode_fault(-32601, "unsupported");
        let document = Document::parse(&xml).expect("a well-formed fault");
        let members: Vec<(String, String)> = document
            .descendants()
            .filter(|node| node.has_tag_name("member"))
            .map(|member| {
                let name = member
                    .children()
                    .find(|child| child.has_tag_name("name"))
                    .and_then(|name| name.text())
                    .unwrap_or_default()
                    .to_owned();
                let value = member
                    .children()
                    .find(|child| child.has_tag_name("value"))
                    .map(
                        |value| match value.children().find(roxmltree::Node::is_element) {
                            // A typed value, `<int>-32601</int>`, has its text one level down.
                            Some(typed) => typed.text().unwrap_or_default().to_owned(),
                            None => value.text().unwrap_or_default().to_owned(),
                        },
                    )
                    .unwrap_or_default();
                (name, value)
            })
            .collect();
        assert_eq!(
            members,
            vec![
                ("faultCode".to_owned(), "-32601".to_owned()),
                ("faultString".to_owned(), "unsupported".to_owned())
            ]
        );
    }
}

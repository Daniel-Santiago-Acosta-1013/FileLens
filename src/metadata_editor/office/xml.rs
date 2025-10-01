use xmltree::{Element, XMLNode};

use crate::metadata_editor::constants::{APP_NS, CP_NS, DC_NS, DCTERMS_NS};

/// Describe la información necesaria para localizar un nodo en el XML de propiedades.
#[derive(Clone, Copy)]
pub(crate) struct FieldSpec<'a> {
    pub(crate) prefix: Option<&'a str>,
    pub(crate) local_name: &'a str,
    pub(crate) namespace: Option<&'a str>,
}

/// Obtiene el campo correspondiente en `core.xml` a partir de su etiqueta declarada.
pub(crate) fn core_field_spec(tag: &str) -> Option<FieldSpec<'static>> {
    match tag {
        "dc:creator" => Some(FieldSpec {
            prefix: Some("dc"),
            local_name: "creator",
            namespace: Some(DC_NS),
        }),
        "cp:lastModifiedBy" => Some(FieldSpec {
            prefix: Some("cp"),
            local_name: "lastModifiedBy",
            namespace: Some(CP_NS),
        }),
        "dcterms:created" => Some(FieldSpec {
            prefix: Some("dcterms"),
            local_name: "created",
            namespace: Some(DCTERMS_NS),
        }),
        "dcterms:modified" => Some(FieldSpec {
            prefix: Some("dcterms"),
            local_name: "modified",
            namespace: Some(DCTERMS_NS),
        }),
        "dc:title" => Some(FieldSpec {
            prefix: Some("dc"),
            local_name: "title",
            namespace: Some(DC_NS),
        }),
        "dc:subject" => Some(FieldSpec {
            prefix: Some("dc"),
            local_name: "subject",
            namespace: Some(DC_NS),
        }),
        "dc:description" => Some(FieldSpec {
            prefix: Some("dc"),
            local_name: "description",
            namespace: Some(DC_NS),
        }),
        "cp:keywords" => Some(FieldSpec {
            prefix: Some("cp"),
            local_name: "keywords",
            namespace: Some(CP_NS),
        }),
        "cp:category" => Some(FieldSpec {
            prefix: Some("cp"),
            local_name: "category",
            namespace: Some(CP_NS),
        }),
        "cp:contentStatus" => Some(FieldSpec {
            prefix: Some("cp"),
            local_name: "contentStatus",
            namespace: Some(CP_NS),
        }),
        "cp:revision" => Some(FieldSpec {
            prefix: Some("cp"),
            local_name: "revision",
            namespace: Some(CP_NS),
        }),
        _ => None,
    }
}

/// Obtiene el campo correspondiente en `app.xml` a partir de su etiqueta declarada.
pub(crate) fn app_field_spec(tag: &str) -> Option<FieldSpec<'static>> {
    match tag {
        "Application" => Some(FieldSpec {
            prefix: None,
            local_name: "Application",
            namespace: Some(APP_NS),
        }),
        "Company" => Some(FieldSpec {
            prefix: None,
            local_name: "Company",
            namespace: Some(APP_NS),
        }),
        "Manager" => Some(FieldSpec {
            prefix: None,
            local_name: "Manager",
            namespace: Some(APP_NS),
        }),
        "Pages" => Some(FieldSpec {
            prefix: None,
            local_name: "Pages",
            namespace: Some(APP_NS),
        }),
        "Words" => Some(FieldSpec {
            prefix: None,
            local_name: "Words",
            namespace: Some(APP_NS),
        }),
        "Lines" => Some(FieldSpec {
            prefix: None,
            local_name: "Lines",
            namespace: Some(APP_NS),
        }),
        _ => None,
    }
}

/// Inserta o sustituye el contenido de un elemento de metadata.
pub(crate) fn apply_update_to_element(
    root: &mut Element,
    spec: FieldSpec<'_>,
    new_value: &str,
) -> bool {
    for node in root.children.iter_mut() {
        if let XMLNode::Element(child) = node
            && element_matches(child, &spec)
        {
            return set_element_text(child, new_value);
        }
    }

    let mut new_child = Element::new(spec.local_name);
    if let Some(prefix) = spec.prefix {
        new_child.prefix = Some(prefix.to_string());
    }
    if let Some(namespace) = spec.namespace {
        new_child.namespace = Some(namespace.to_string());
    }
    if !new_value.is_empty() {
        new_child
            .children
            .push(XMLNode::Text(new_value.to_string()));
    }
    root.children.push(XMLNode::Element(new_child));
    true
}

/// Comprueba si un elemento coincide con la especificación de búsqueda.
pub(crate) fn element_matches(element: &Element, spec: &FieldSpec<'_>) -> bool {
    if element.name != spec.local_name {
        return false;
    }

    match (spec.namespace, element.namespace.as_deref()) {
        (Some(expected), Some(actual)) => expected == actual,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

/// Sustituye el texto de un elemento si difiere del valor actual.
pub(crate) fn set_element_text(element: &mut Element, new_value: &str) -> bool {
    let current = element
        .children
        .iter()
        .find_map(|node| match node {
            XMLNode::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .unwrap_or("");
    if current == new_value {
        return false;
    }

    element
        .children
        .retain(|node| !matches!(node, XMLNode::Text(_)));

    if !new_value.is_empty() {
        element.children.push(XMLNode::Text(new_value.to_string()));
    }

    true
}

/// Devuelve el texto plano contenido dentro de un elemento.
pub(crate) fn element_text_content(element: &Element) -> String {
    let mut content = String::new();
    for node in &element.children {
        if let XMLNode::Text(text) = node {
            content.push_str(text);
        }
    }
    content.trim().to_string()
}

/// Comprueba que el contenido almacenado en un elemento coincide con el valor esperado.
pub(crate) fn element_matches_expected_value(
    root: &Element,
    spec: FieldSpec<'_>,
    expected: &str,
) -> bool {
    for node in &root.children {
        if let XMLNode::Element(child) = node
            && element_matches(child, &spec)
        {
            return element_text_content(child) == expected;
        }
    }
    expected.is_empty()
}

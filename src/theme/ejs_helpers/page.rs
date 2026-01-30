//! Page type checking helpers
//!
//! These functions check the current page type (is_home, is_post, etc.)

use crate::theme::ejs::EjsValue;

/// Check if page has a specific boolean property
pub fn check_page_property(page: Option<&EjsValue>, property: &str) -> bool {
    if let Some(EjsValue::Object(page_obj)) = page {
        if let Some(EjsValue::Bool(val)) = page_obj.get(property) {
            return *val;
        }
    }
    false
}

/// Check if current page is home
pub fn is_home(page: Option<&EjsValue>) -> bool {
    check_page_property(page, "is_home")
}

/// Check if current page is a post
pub fn is_post(page: Option<&EjsValue>) -> bool {
    // Check explicit is_post property or layout == "post"
    if check_page_property(page, "is_post") {
        return true;
    }
    if let Some(EjsValue::Object(page_obj)) = page {
        if let Some(EjsValue::String(layout)) = page_obj.get("layout") {
            return layout == "post";
        }
    }
    false
}

/// Check if current page is a page (not post)
pub fn is_page(page: Option<&EjsValue>) -> bool {
    if check_page_property(page, "is_page") {
        return true;
    }
    if let Some(EjsValue::Object(page_obj)) = page {
        if let Some(EjsValue::String(layout)) = page_obj.get("layout") {
            return layout == "page";
        }
    }
    false
}

/// Check if current page is archive
pub fn is_archive(page: Option<&EjsValue>) -> bool {
    check_page_property(page, "is_archive")
}

/// Check if current page is category
pub fn is_category(page: Option<&EjsValue>) -> bool {
    check_page_property(page, "is_category")
}

/// Check if current page is tag
pub fn is_tag(page: Option<&EjsValue>) -> bool {
    check_page_property(page, "is_tag")
}

/// Check if current page is year archive
pub fn is_year(page: Option<&EjsValue>) -> bool {
    if let Some(EjsValue::Object(page_obj)) = page {
        // Has year but no month
        let has_year = page_obj
            .get("year")
            .map(|v| !matches!(v, EjsValue::Null))
            .unwrap_or(false);
        let has_month = page_obj
            .get("month")
            .map(|v| !matches!(v, EjsValue::Null))
            .unwrap_or(false);
        return has_year && !has_month;
    }
    false
}

/// Check if current page is month archive
pub fn is_month(page: Option<&EjsValue>) -> bool {
    if let Some(EjsValue::Object(page_obj)) = page {
        let has_month = page_obj
            .get("month")
            .map(|v| !matches!(v, EjsValue::Null))
            .unwrap_or(false);
        return has_month;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    #[test]
    fn test_is_home() {
        let mut page = IndexMap::new();
        page.insert("is_home".to_string(), EjsValue::Bool(true));
        assert!(is_home(Some(&EjsValue::Object(page))));
    }

    #[test]
    fn test_is_post() {
        let mut page = IndexMap::new();
        page.insert("layout".to_string(), EjsValue::String("post".to_string()));
        assert!(is_post(Some(&EjsValue::Object(page))));
    }
}

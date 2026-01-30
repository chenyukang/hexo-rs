//! Array manipulation helpers: sort, limit, slice, filter, reverse

use crate::theme::ejs::EjsValue;

/// Sort an array of objects by a property
/// Returns a new sorted vector
pub fn sort(arr: &[EjsValue], property: &str, descending: bool) -> Vec<EjsValue> {
    let mut sorted: Vec<EjsValue> = arr.to_vec();
    sorted.sort_by(|a, b| {
        let a_val = a
            .get_property(property)
            .map(|v| v.to_output_string())
            .unwrap_or_default();
        let b_val = b
            .get_property(property)
            .map(|v| v.to_output_string())
            .unwrap_or_default();

        let cmp = a_val.cmp(&b_val);
        if descending {
            cmp.reverse()
        } else {
            cmp
        }
    });
    sorted
}

/// Limit array to first n items
pub fn limit(arr: &[EjsValue], n: usize) -> Vec<EjsValue> {
    arr.iter().take(n).cloned().collect()
}

/// Slice array from start to end
pub fn slice(arr: &[EjsValue], start: usize, end: usize) -> Vec<EjsValue> {
    arr.iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .cloned()
        .collect()
}

/// Reverse an array
pub fn reverse(arr: &[EjsValue]) -> Vec<EjsValue> {
    arr.iter().rev().cloned().collect()
}

/// Get count/length of array
pub fn count(arr: &[EjsValue]) -> usize {
    arr.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    #[test]
    fn test_limit() {
        let arr = vec![
            EjsValue::Number(1.0),
            EjsValue::Number(2.0),
            EjsValue::Number(3.0),
        ];
        let result = limit(&arr, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_slice() {
        let arr = vec![
            EjsValue::Number(1.0),
            EjsValue::Number(2.0),
            EjsValue::Number(3.0),
            EjsValue::Number(4.0),
        ];
        let result = slice(&arr, 1, 3);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_reverse() {
        let arr = vec![
            EjsValue::Number(1.0),
            EjsValue::Number(2.0),
            EjsValue::Number(3.0),
        ];
        let result = reverse(&arr);
        assert_eq!(result[0].to_output_string(), "3");
    }

    #[test]
    fn test_sort() {
        let mut obj1 = IndexMap::new();
        obj1.insert(
            "date".to_string(),
            EjsValue::String("2024-01-01".to_string()),
        );

        let mut obj2 = IndexMap::new();
        obj2.insert(
            "date".to_string(),
            EjsValue::String("2024-02-01".to_string()),
        );

        let arr = vec![EjsValue::Object(obj1), EjsValue::Object(obj2)];

        // Sort ascending
        let sorted = sort(&arr, "date", false);
        assert!(
            sorted[0].get_property("date").unwrap().to_output_string()
                < sorted[1].get_property("date").unwrap().to_output_string()
        );

        // Sort descending
        let sorted_desc = sort(&arr, "date", true);
        assert!(
            sorted_desc[0]
                .get_property("date")
                .unwrap()
                .to_output_string()
                > sorted_desc[1]
                    .get_property("date")
                    .unwrap()
                    .to_output_string()
        );
    }
}

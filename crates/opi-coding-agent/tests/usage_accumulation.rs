use opi_ai::stream::{CumulativeUsage, Usage};

#[test]
fn cumulative_usage_accumulates() {
    let mut cu = CumulativeUsage::default();
    cu.accumulate(&Usage {
        input_tokens: 100,
        output_tokens: 50,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    });
    assert_eq!(cu.total_input_tokens(), 100);
    assert_eq!(cu.total_output_tokens(), 50);
    assert_eq!(cu.turn_count(), 1);

    cu.accumulate(&Usage {
        input_tokens: 200,
        output_tokens: 75,
        cache_read_tokens: 10,
        cache_write_tokens: 5,
    });
    assert_eq!(cu.total_input_tokens(), 300);
    assert_eq!(cu.total_output_tokens(), 125);
    assert_eq!(cu.turn_count(), 2);
}

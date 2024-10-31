use alloy_primitives::{hex, Bytes};

fn format_test_file(test_names: String, call_data: String, test_size: usize) -> String {
    format!(
        r#"
// SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.15;

contract MulticallerCallData  {{

    string[{}] testname = [
{}
];

    bytes[{}] callsdata = [
{}
];


    function get_call_data(uint256 i ) internal returns (bytes memory) {{
        return callsdata[i];
    }}

    function get_test_name(uint256 i ) internal returns (string memory) {{
        return testname[i];
    }}

}}
        "#,
        test_size, test_names, test_size, call_data
    )
}

pub fn create_sol_test(requests_vec: Vec<(String, Bytes)>) -> String {
    let requests_vec = requests_vec;
    let req_len = requests_vec.len();
    let (names, data_vec): (Vec<String>, Vec<Bytes>) = requests_vec.into_iter().unzip();
    let names_string_vec: Vec<String> = names.into_iter().map(|x| format!("\t\"{}\"", x)).collect();
    let names_string = names_string_vec.join(",\n");
    let calldata_string_vec: Vec<String> = data_vec.into_iter().map(|x| format!("\tbytes(hex\"{}\")", hex::encode(x))).collect();
    let calldata_string = calldata_string_vec.join(",\n");
    format_test_file(names_string, calldata_string, req_len)
}

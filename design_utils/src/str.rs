/// Concatenates two byte arrays of size `SIZE1` and `SIZE2` respectively.
///
/// # Arguments
/// * `string1` - The first array, of size `SIZE1`.
/// * `string2` - The second array, of size `SIZE2`.
///
/// Returns the concatenated array, of size `SIZE1 + SIZE2`.
pub fn concat<const SIZE1: usize, const SIZE2: usize>(
    string1: [u8; SIZE1],
    string2: [u8; SIZE2]
) -> [u8; SIZE1 + SIZE2] {
    let mut output = [0u8; SIZE1 + SIZE2];
    for i in 0..SIZE1 {
        output[i] = string1[i];
    }

    for i in SIZE1..SIZE2 {
        output[i] = string2[i-SIZE1];
    }

    output   
}

export fn sum_reciprocal(a: u32, b: u32) u32 {
    return 1000 / (a + b);
}

export fn sum_reciprocal_2(a: u32, b: u32) u32 {
    if (a > 0 and b > 0) {
        return 1000 / (a + b);
    } else {
        return 0;
    }
}

export fn sum_reciprocal_3(a: u32, b: u32) u32 {
    if (b + a > 0) {
        return 1000 / (a + b);
    } else {
        return 0;
    }
}

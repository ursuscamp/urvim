// Zig syntax fixture
const std = @import("std");
pub const answer: u32 = 42;
pub const flag = true;
pub const message = "hello";
std.debug.print("{s}\n", .{message});
pub fn main() void { }

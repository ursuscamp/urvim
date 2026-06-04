// Zig syntax fixture
// Multi-line comment
// continues here

const std = @import("std");
const print = std.debug.print;
const Allocator = std.mem.Allocator;
const ArrayList = std.ArrayList;

pub const answer: u32 = 42;
pub const flag = true;
pub const message = "hello";
pub const floating = 3.14;
pub const hex_val = 0xFF;
pub const binary_val = 0b1010_0011;
pub const octal_val = 0o77;

pub const Point = struct {
    x: f64,
    y: f64,

    pub fn init(x: f64, y: f64) Point {
        return Point{ .x = x, .y = y };
    }

    pub fn magnitude(self: Point) f64 {
        return @sqrt(self.x * self.x + self.y * self.y);
    }
};

pub const Color = enum {
    Red,
    Green,
    Blue,
};

pub const MaybeInt = union(enum) {
    Some: i32,
    None,
};

pub fn main() void {
    const p = Point.init(3.0, 4.0);
    const mag = p.magnitude();
    print("magnitude: {d}\n", .{mag});

    var arr = ArrayList(u8).init(std.heap.page_allocator);
    defer arr.deinit();
    arr.appendSlice("hello") catch return;

    const slice: []const u8 = "hello world";
    for (slice) |byte| {
        print("{c}", .{byte});
    }
    print("\n", .{});

    comptime {
        const x = 42;
        if (x > 0) {
            print("comptime works\n", .{});
        }
    }

    inline for (.{1, 2, 3}) |val| {
        print("val: {d}\n", .{val});
    }
}

// C++ syntax fixture
#include <vector>
namespace demo {
template <typename T>
class Box {
public:
  explicit Box(T value) : value(value) {}
  T value;
};

auto make_box() -> Box<int> {
  return Box<int>{42};
}

const char* raw = R"(hello
world)";
constexpr bool enabled = true;
auto none = nullptr;
std::printf("value=%d\n", 42);
std::fprintf(stderr, "%s %s", "first", "second");
}

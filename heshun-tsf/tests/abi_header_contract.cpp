#include "heshun.h"
#include <cstddef>
#include <cstdint>
#include <type_traits>

static_assert(std::is_standard_layout<hs_text_view>::value, "hs_text_view must be standard layout");
static_assert(std::is_standard_layout<hs_candidate_view>::value, "hs_candidate_view must be standard layout");
static_assert(std::is_standard_layout<hs_runtime_event_t>::value, "hs_runtime_event_t must be standard layout");
static_assert(std::is_standard_layout<hs_runtime_result>::value, "hs_runtime_result must be standard layout");
static_assert(sizeof(((hs_text_view*)nullptr)->len) == sizeof(std::uint32_t), "text length must be u32");
static_assert(sizeof(((hs_runtime_event_t*)nullptr)->opcode) == sizeof(std::uint32_t), "opcode must be u32");
static_assert(sizeof(((hs_runtime_event_t*)nullptr)->ordinal) == sizeof(std::uint32_t), "ordinal must be u32");

int main() {
    hs_runtime_event_t event{};
    event.opcode = 0;
    event.value = static_cast<long long>('a');
    hs_runtime_result* result = nullptr;
    (void)result;
    return event.value == 'a' ? 0 : 1;
}

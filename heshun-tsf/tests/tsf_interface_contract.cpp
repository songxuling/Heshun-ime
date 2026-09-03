#include "text_service.h"
#include "candidate_list.h"
#include <type_traits>

static_assert(std::is_base_of<ITfTextEditSink, HeshunTextService>::value,
              "HeshunTextService must implement ITfTextEditSink");
static_assert(std::is_base_of<ITfTextLayoutSink, HeshunTextService>::value,
              "HeshunTextService must implement ITfTextLayoutSink");
static_assert(std::is_base_of<ITfThreadFocusSink, HeshunTextService>::value,
              "HeshunTextService must implement ITfThreadFocusSink");
static_assert(std::is_base_of<ITfCandidateListUIElement, HeshunCandidateList>::value,
              "candidate adapter must implement ITfCandidateListUIElement");
static_assert(std::is_base_of<ITfCandidateListUIElementBehavior, HeshunCandidateList>::value,
              "candidate adapter must implement ITfCandidateListUIElementBehavior");
static_assert(std::is_base_of<ITfIntegratableCandidateListUIElement, HeshunCandidateList>::value,
              "candidate adapter must implement ITfIntegratableCandidateListUIElement");

int main() {
    return 0;
}
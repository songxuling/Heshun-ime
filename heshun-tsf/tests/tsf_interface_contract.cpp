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
    const POINT tsf{640, 480};
    const POINT gui{12, 24};
    const POINT fallback{0, 0};
    const POINT preferred = ResolveCandidateAnchorPoint(true, tsf, true, gui, fallback);
    if (preferred.x != tsf.x || preferred.y != tsf.y) return 1;
    const POINT gui_only = ResolveCandidateAnchorPoint(false, tsf, true, gui, fallback);
    if (gui_only.x != gui.x || gui_only.y != gui.y) return 2;
    const POINT default_only = ResolveCandidateAnchorPoint(false, tsf, false, gui, fallback);
    if (default_only.x != fallback.x || default_only.y != fallback.y) return 3;
    return 0;
}
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

    if (CandidateWindowTopForAnchor(400, 432, 180, 0, 1000, 4) != 436) return 4;
    if (CandidateWindowTopForAnchor(900, 932, 180, 0, 1000, 4) != 716) return 5;
    if (CandidateWindowTopForAnchor(990, 1022, 180, 0, 1000, 4) != 806) return 6;
    if (CandidateWindowTopForAnchor(20, 52, 180, 0, 100, 4) != 0) return 7;

    RECT outside{810, 1272, 810, 1304};
    RECT foreground{100, 200, 900, 900};
    const RECT corrected = CorrectCandidateAnchorRect(outside, foreground, true, POINT{20, 30});
    if (corrected.left != 120 || corrected.top != 230 || corrected.bottom != 262) return 8;
    RECT inside{300, 400, 300, 432};
    const RECT unchanged = CorrectCandidateAnchorRect(inside, foreground, true, POINT{20, 30});
    if (unchanged.left != inside.left || unchanged.top != inside.top) return 9;

    if (CandidateIndexFromNumberKey('1', 9) != 0) return 10;
    if (CandidateIndexFromNumberKey('9', 9) != 8) return 11;
    if (CandidateIndexFromNumberKey('9', 8) != -1) return 12;
    if (CandidateIndexFromNumberKey('0', 9) != -1) return 13;
    if (CandidateIndexFromNumberKey(VK_NUMPAD1, 9) != -1) return 14;
    if (CandidateWindowContentHeight(9, 25, 8) != 241) return 15;
    if (CandidateWindowRowIndexAtY(8, 25, 8, 9) != 0) return 16;
    if (CandidateWindowRowIndexAtY(32, 25, 8, 9) != 0) return 17;
    if (CandidateWindowRowIndexAtY(33, 25, 8, 9) != 1) return 18;
    if (CandidateWindowRowIndexAtY(233, 25, 8, 9) != 9) return 19;

    if (CandidateWindowContentHeight(9, 30, 10) != 290) return 20;
    if (DynamicCandidateClientWidth(80, 30, 10, 180, 700) != 180) return 21;
    if (DynamicCandidateClientWidth(320, 30, 10, 180, 700) != 360) return 22;
    if (DynamicCandidateClientWidth(800, 30, 10, 180, 1200) != 840) return 23;
    if (DynamicCandidateClientWidth(1400, 30, 10, 180, 1200) != 1200) return 24;
    return 0;
}
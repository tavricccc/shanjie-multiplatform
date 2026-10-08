#include "mode_indicator.h"
extern "C" { void* sj_indicator_new();void sj_indicator_free(void*);void sj_indicator_hide(void*);void sj_indicator_show(void*,RECT,unsigned,bool); }
ModeIndicator::ModeIndicator():handle_(sj_indicator_new()){} ModeIndicator::~ModeIndicator(){sj_indicator_free(handle_);}
void ModeIndicator::hide(){sj_indicator_hide(handle_);}void ModeIndicator::show(const RECT& caret,UINT dpi,bool chinese){sj_indicator_show(handle_,caret,dpi,chinese);}

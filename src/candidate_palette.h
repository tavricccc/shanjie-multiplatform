#pragma once
#include <d2d1.h>
struct CandidatePalette { D2D1_COLOR_F background,border,text,accent,selectedText; };
CandidatePalette candidatePalette(unsigned theme);

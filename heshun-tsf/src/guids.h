#pragma once

#include <windows.h>
#include <msctf.h>

// {B637A70D-049C-4611-9E0C-932E346BC3D2}
inline constexpr CLSID CLSID_HeshunTextService =
{ 0xb637a70d, 0x049c, 0x4611, { 0x9e, 0x0c, 0x93, 0x2e, 0x34, 0x6b, 0xc3, 0xd2 } };

// {6802378F-5C83-4CA2-8388-B4C3467E9CF8}
inline constexpr GUID GUID_PROFILE_HESHUN_ZHENGMA =
{ 0x6802378f, 0x5c83, 0x4ca2, { 0x83, 0x88, 0xb4, 0xc3, 0x46, 0x7e, 0x9c, 0xf8 } };

// {8EAA7BFE-2D4C-4A83-8DFE-7D9B3E0C8F8A}
inline constexpr GUID GUID_COMPARTMENT_HESHUN_SERVICE =
{ 0x8eaa7bfe, 0x2d4c, 0x4a83, { 0x8d, 0xfe, 0x7d, 0x9b, 0x3e, 0x0c, 0x8f, 0x8a } };

inline constexpr wchar_t kHeshunServiceName[] = L"heshun 郑码";
inline constexpr LANGID kHeshunLangId = MAKELANGID(LANG_CHINESE, SUBLANG_CHINESE_SIMPLIFIED);

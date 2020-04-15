#ifndef _WXSR_HPP_
#define _WXSR_HPP_

#include <wx/string.h>

namespace sr
{
	const wxString TITLE = wxT("OpenPal3 启动器");
	const wxString PATH_CORRECT = wxT("✔路径正确");
	const wxString PATH_INCORRECT = wxT("❌路径不正确，请重新指定");
	const wxString OPENPAL3_CONFIG_LOAD_FAILED = wxT("读取文件配置失败，将使用默认配置覆盖当前配置文件。是否继续？");
	const wxString START_PROCESS_FAILED_TEMPLATE = wxT("无法启动 OpenPAL3： %ws");
}

#endif

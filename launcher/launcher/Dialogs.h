///////////////////////////////////////////////////////////////////////////
// C++ code generated with wxFormBuilder (version Nov  6 2017)
// http://www.wxformbuilder.org/
//
// PLEASE DO *NOT* EDIT THIS FILE!
///////////////////////////////////////////////////////////////////////////

#ifndef __DIALOGS_H__
#define __DIALOGS_H__

#include <wx/artprov.h>
#include <wx/xrc/xmlres.h>
#include <wx/string.h>
#include <wx/stattext.h>
#include <wx/gdicmn.h>
#include <wx/font.h>
#include <wx/colour.h>
#include <wx/settings.h>
#include <wx/filepicker.h>
#include <wx/button.h>
#include <wx/sizer.h>
#include <wx/panel.h>
#include <wx/bitmap.h>
#include <wx/image.h>
#include <wx/icon.h>
#include <wx/notebook.h>
#include <wx/dialog.h>

///////////////////////////////////////////////////////////////////////////


///////////////////////////////////////////////////////////////////////////////
/// Class MainDialog
///////////////////////////////////////////////////////////////////////////////
class MainDialog : public wxDialog 
{
	private:
	
	protected:
		wxNotebook* m_notebook1;
		wxPanel* m_panel1;
		wxStaticText* m_staticText1;
		wxDirPickerCtrl* mPal3DirPicker;
		wxStaticText* mPathValidLabel;
		wxButton* mBtnStartOpenPal3;
		wxButton* mBtnExit;
		
		// Virtual event handlers, overide them in your derived class
		virtual void OnClose( wxCloseEvent& event ) { event.Skip(); }
		virtual void OnInitDialog( wxInitDialogEvent& event ) { event.Skip(); }
		virtual void OnPal3DirChanged( wxFileDirPickerEvent& event ) { event.Skip(); }
		virtual void OnStartOpenPal3Clicked( wxCommandEvent& event ) { event.Skip(); }
		virtual void OnBtnExitClicked( wxCommandEvent& event ) { event.Skip(); }
		
	
	public:
		
		MainDialog( wxWindow* parent, wxWindowID id = wxID_ANY, const wxString& title = wxT("OpenPAL3 启动器"), const wxPoint& pos = wxDefaultPosition, const wxSize& size = wxSize( 439,359 ), long style = wxDEFAULT_DIALOG_STYLE|wxMINIMIZE_BOX|wxSYSTEM_MENU ); 
		~MainDialog();
	
};

#endif //__DIALOGS_H__

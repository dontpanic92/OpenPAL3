///////////////////////////////////////////////////////////////////////////
// C++ code generated with wxFormBuilder (version Nov  6 2017)
// http://www.wxformbuilder.org/
//
// PLEASE DO *NOT* EDIT THIS FILE!
///////////////////////////////////////////////////////////////////////////

#include "Dialogs.h"

///////////////////////////////////////////////////////////////////////////

MainDialog::MainDialog( wxWindow* parent, wxWindowID id, const wxString& title, const wxPoint& pos, const wxSize& size, long style ) : wxDialog( parent, id, title, pos, size, style )
{
	this->SetSizeHints( wxSize( 439,359 ), wxDefaultSize );
	
	wxBoxSizer* bSizer1;
	bSizer1 = new wxBoxSizer( wxVERTICAL );
	
	m_notebook1 = new wxNotebook( this, wxID_ANY, wxDefaultPosition, wxDefaultSize, 0 );
	m_panel1 = new wxPanel( m_notebook1, wxID_ANY, wxDefaultPosition, wxDefaultSize, wxTAB_TRAVERSAL );
	wxBoxSizer* bSizer2;
	bSizer2 = new wxBoxSizer( wxVERTICAL );
	
	m_staticText1 = new wxStaticText( m_panel1, wxID_ANY, wxT("安装路径："), wxDefaultPosition, wxDefaultSize, 0 );
	m_staticText1->Wrap( -1 );
	bSizer2->Add( m_staticText1, 0, wxALL, 5 );
	
	mPal3DirPicker = new wxDirPickerCtrl( m_panel1, wxID_ANY, wxEmptyString, wxT("选择仙剑奇侠传三安装路径"), wxDefaultPosition, wxDefaultSize, wxDIRP_DEFAULT_STYLE );
	bSizer2->Add( mPal3DirPicker, 0, wxALL|wxEXPAND, 5 );
	
	mPathValidLabel = new wxStaticText( m_panel1, wxID_ANY, wxEmptyString, wxDefaultPosition, wxDefaultSize, 0 );
	mPathValidLabel->Wrap( -1 );
	bSizer2->Add( mPathValidLabel, 0, wxALL, 5 );
	
	
	bSizer2->Add( 0, 0, 1, wxEXPAND, 5 );
	
	mBtnStartOpenPal3 = new wxButton( m_panel1, wxID_ANY, wxT("开始游戏"), wxDefaultPosition, wxDefaultSize, 0 );
	bSizer2->Add( mBtnStartOpenPal3, 0, wxALL|wxEXPAND, 5 );
	
	
	m_panel1->SetSizer( bSizer2 );
	m_panel1->Layout();
	bSizer2->Fit( m_panel1 );
	m_notebook1->AddPage( m_panel1, wxT("仙剑奇侠传三"), false );
	
	bSizer1->Add( m_notebook1, 1, wxEXPAND | wxALL, 5 );
	
	wxBoxSizer* bSizer4;
	bSizer4 = new wxBoxSizer( wxHORIZONTAL );
	
	mBtnExit = new wxButton( this, wxID_ANY, wxT("退出"), wxDefaultPosition, wxDefaultSize, 0 );
	bSizer4->Add( mBtnExit, 0, wxALL, 5 );
	
	
	bSizer1->Add( bSizer4, 0, wxALIGN_RIGHT, 5 );
	
	
	this->SetSizer( bSizer1 );
	this->Layout();
	
	this->Centre( wxBOTH );
	
	// Connect Events
	this->Connect( wxEVT_CLOSE_WINDOW, wxCloseEventHandler( MainDialog::OnClose ) );
	this->Connect( wxEVT_INIT_DIALOG, wxInitDialogEventHandler( MainDialog::OnInitDialog ) );
	mPal3DirPicker->Connect( wxEVT_COMMAND_DIRPICKER_CHANGED, wxFileDirPickerEventHandler( MainDialog::OnPal3DirChanged ), NULL, this );
	mBtnStartOpenPal3->Connect( wxEVT_COMMAND_BUTTON_CLICKED, wxCommandEventHandler( MainDialog::OnStartOpenPal3Clicked ), NULL, this );
	mBtnExit->Connect( wxEVT_COMMAND_BUTTON_CLICKED, wxCommandEventHandler( MainDialog::OnBtnExitClicked ), NULL, this );
}

MainDialog::~MainDialog()
{
	// Disconnect Events
	this->Disconnect( wxEVT_CLOSE_WINDOW, wxCloseEventHandler( MainDialog::OnClose ) );
	this->Disconnect( wxEVT_INIT_DIALOG, wxInitDialogEventHandler( MainDialog::OnInitDialog ) );
	mPal3DirPicker->Disconnect( wxEVT_COMMAND_DIRPICKER_CHANGED, wxFileDirPickerEventHandler( MainDialog::OnPal3DirChanged ), NULL, this );
	mBtnStartOpenPal3->Disconnect( wxEVT_COMMAND_BUTTON_CLICKED, wxCommandEventHandler( MainDialog::OnStartOpenPal3Clicked ), NULL, this );
	mBtnExit->Disconnect( wxEVT_COMMAND_BUTTON_CLICKED, wxCommandEventHandler( MainDialog::OnBtnExitClicked ), NULL, this );
	
}

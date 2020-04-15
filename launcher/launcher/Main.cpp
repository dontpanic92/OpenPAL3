// Main.cpp : Defines the entry point for the application.
//

#include <wx/wx.h>
#include <wx/aboutdlg.h>
#include "Dialogs.h"
#include "OpenPal3Config.hpp"
#include "wxSR.hpp"

using namespace std;

class MainDialogImpl : public MainDialog
{
public:
    MainDialogImpl() : MainDialog(nullptr) {}

protected:
    virtual void OnInitDialog(wxInitDialogEvent& event) override
    {
        try
        {
            this->mConfig.Load();
        }
        catch (std::exception&)
        {
            if (wxNO == wxMessageBox(sr::OPENPAL3_CONFIG_LOAD_FAILED, sr::TITLE, wxYES_NO))
            {
                this->Close();
            }
        }

        auto path = this->mConfig.GetAssetPath();
        this->mPal3DirPicker->SetPath(wxString::FromUTF8(path));
        this->ValidatePal3Dir(wxString::FromUTF8(path));
    }

    virtual void OnClose(wxCloseEvent& event) override
    { 
        this->Destroy();
    }

    void OnPal3DirChanged(wxFileDirPickerEvent& evt) override
    {
        auto path = evt.GetPath();
        if (this->ValidatePal3Dir(path))
        {
            this->mConfig.SetAssetPath(path.ToStdString(wxConvUTF8));
            this->mConfig.Save();
        }
    }

    void OnStartOpenPal3Clicked(wxCommandEvent& event) override 
    {
#ifdef WIN32
        long long ret = reinterpret_cast<long long>(ShellExecuteW(NULL, L"open", L"openpal3", NULL, NULL, SW_SHOWDEFAULT));
        if (ret <= 32)
        {
            wchar_t buf[256];
            FormatMessageW(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
                NULL, GetLastError(), MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT),
                buf, (sizeof(buf) / sizeof(wchar_t)), NULL);
            wxMessageBox(wxString::Format(sr::START_PROCESS_FAILED_TEMPLATE, buf), sr::TITLE);
        }
        else
        {
            this->Close();
        }
#endif
    }

    void OnBtnExitClicked(wxCommandEvent& event) override
    { 
        this->Close();
    }

private:
    bool ValidatePal3Dir(wxString dirPath)
    {
        bool valid = !dirPath.IsEmpty();
        this->mBtnStartOpenPal3->Enable(valid);
        this->mPathValidLabel->SetLabelText(valid ? sr::PATH_CORRECT : sr::PATH_INCORRECT);
        
        return valid;
    }

    OpenPal3Config mConfig;
};

class MyApp : public wxApp
{
public:
    virtual bool OnInit() override 
    { 
#ifdef WIN32
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
#endif
        auto dialog = new MainDialogImpl(); 
        dialog->Show();
        return true; 
    }
};

wxIMPLEMENT_APP(MyApp);

use std::libc;
use std::os::raw::c_char;
use winapi::HANDLE;
use lazy_static::*;

pub enum CPKTableFlag {
    CPKTableFlag_None = 0x0,
    CPKTableFlag_IsFile = 0x1,         //是否是合法文件？
    CPKTableFlag_IsDir = 0x2,          //是否是目录
    CPKTableFlag_Unknown2 = 0x4,
    CPKTableFlag_Unknown3 = 0x8,
    CPKTableFlag_IsDeleted = 0x10,     //是否已删除
}

struct CPKTable {
    dwCRC: winapi::DWORD,                //0x00  当前节点CRC
    dwFlag:  CPKTableFlag,          //0x04  文件属性信息
    dwFatherCRC: winapi::DWORD,          //0x08  父节点CRC，根节点为0
    dwStartPos: winapi::DWORD,           //0x0C  压缩后的数据在CPK中的偏移量。
    dwPackedSize: winapi::DWORD,         //0x10  压缩后数据的大小。对于目录，这个值为0。
    dwOriginSize: winapi::DWORD,         //0x14  原始文件的大小，方便你解压时开缓冲区。
    dwExtraInfoSize: winapi::DWORD,      //0x18  文件名信息偏移 信息紧跟着压缩数据之后，所以从dwStartPos + dwPackedSize读取dwExtraInfoSize，最开头就是文件名
}


pub struct CPKFile {
        bValid: bool,                 //0x110 是否有效
        dwCRC: winapi::DWORD,              //0x114 
        dwFatherCRC: winapi::DWORD,        //0x118 父节点CRC
        nTableIndex: winapi::DWORD,        //0x11C 文件数组下标
        lpMapAddress: void,           //0x120 文件映射基址
        void: lpStartAddress,         //0x124 文件原始缓冲
        dwOffset: winapi::DWORD,           //0x128 对齐偏移量
        bCompressed: bool,            //0x12C 是否是压缩文件
        lpMem: *mut void,             //0x130 一般存放解压缩内容
        dwFileSize: winapi::DWORD,         //0x134 原始文件大小
        dwPointer: winapi::DWORD,          //0x138 文件指针
        pRecordEntry: *mut CPKTable,  //0x13C 文件结构指针
    }

//0x140
struct gbVFile {
    OpenMode: winapi::DWORD,                 //0x0
    EntryAddr: winapi::DWORD,                //0x4
    FileSize: winapi::DWORD,                 //0x8
    fileName: [::std::os::c_char; 260usize],        //0xC       文件名
    cpkFile: *mut CPKFile,
}

pub struct CpkZipUnzipParam {
    flag: int32_t,                  //0x00  一般为2，从CpkFileEntry::Attrib的HIWORD复制
    bCompress: bool,                //0x04  是否启用压缩
    src: *mut void,                 //0x08  文件源数据指针
    dest: *mut void,                //0x0C  文件目标数据指针
    srcSizeUnused: winapi::DWORD,        //0x10  暂时未发现有使用
    destSize: winapi::DWORD,             //0x14  目标数据大小
    srcSize: winapi::DWORD,              //0x18  源数据大小
    destResultSize: winapi::DWORD,       //0x1C  实际得到的数据大小
    bResult: bool,                  //0x20  操作是否成功
}

//0x80
pub struct CPKHeader {
    dwLable: winapi::DWORD,                  //0x0
    dwVersion: winapi::DWORD,                //0x4   版本 必须为1
    dwTableStart: winapi::DWORD,             //0x08
    dwDataStart: winapi::DWORD,              //0x0C
    dwMaxFileNum: winapi::DWORD,             //0x10  最大文件数量
    dwFileNum: winapi::DWORD,                //0x14  文件数量
    dwIsFormatted: winapi::DWORD,            //0x18
    dwSizeOfHeader: winapi::DWORD,           //0x1C
    dwValidTableNum: winapi::DWORD,          //0x20  CpkFileEntry数组数量
    dwMaxTableNum: winapi::DWORD,            //0x24
    dwFragmentNum: winapi::DWORD,            //0x28
    dwPackageSize: winapi::DWORD,            //0x2C
    dwReserved: [winapi::DWORD; 20usize],    //0x30
}

pub enum ECPKMode {
    CPKM_Null = 0,
    CPKM_Normal = 1,
    CPKM_FileMapping = 2,
    CPKM_Overlapped = 3,
    CPKM_End = 4,
}

pub enum ECPKSeekFileType {
    ECPKSeekFileType_Set = 0,
    ECPKSeekFileType_Add = 1,
    ECPKSeekFileType_Sub = 2,
}


//CPK 全局变量
static CrcTable: [*mut uint32; 256];
static lzo_wrkmem: *mut void;
static g_bCrcTableInitialized: bool = false;

pub struct CPK {

    dwAllocationGranularity: winapi::DWORD,                  //0x0           块对齐长度，做文件映射时需要对齐到该长度，否则映射失败
    m_eMode: ECPKMode,                               //0x4           打开模式 当前为ECPKMode_Mapped
    cpkHeader: CPKHeader,                            //0x8           文件头信息
    entries: [CPKTable;32768],                    //0x88          文件节点信息数组，通过哈希存储
    m_pgbVFile: [*mut gbVFile;0x8],                       //0xE0088       文件数组
    m_bLoaded: bool,                                 //0xE00A8       是否已加载
    m_dwCPKHandle: HANDLE,                           //0xE0090       文件句柄
    m_dwCPKMappingHandle: HANDLE,                    //0xE0094       文件映射句柄
    fileName: [::std::os::c_char;260],                        //0xE0098       CPK文件名
    m_nOpenedFileNum: winapi::DWORD,                         //0xE009C       当前打开的gbVFile文件数量
}

impl CPK {
    pub fn new() -> Self {
        winapi::memset(Box::into_raw(Self),0, sizeof(Self));
        m_eMode = CPKM_FileMapping;
        dwAllocationGranularity = GetAllocationGranularity();
        if !g_bCrcTableInitialized {

        }

    }

    fn GetAllocationGranularity() -> winapi::DWORD
    {
        let SystemInfo: winapi::_SYSTEM_INFO = winapi::_SYSTEM_INFO::new();
        winapi::GetSystemInfo(&SystemInfo);
        return SystemInfo.dwAllocationGranularity;
    }

    fn InitCrcTable() {
        let index: i32 = 0;
        let _crcTable : **mut winapi::DWORD;
        
        if !g_bCrcTableInitialized {

        }
    }

    // pub fn Close(pCpkFile: &CPKFile) -> bool {
    //     if Self.
    //     if pCpkFile.lpMapAddress != nullptr {

    //     }
    // }
}
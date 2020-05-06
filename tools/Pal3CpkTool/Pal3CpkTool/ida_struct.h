#pragma once
/************************************************************************/
/* 供IDA读取的头文件                                                    */
/************************************************************************/

struct CpkFileEntry {
    unsigned int vCRC;              //据我猜测应该是根据文件名Hash出的一个数值，若干个Index结构在CPK文件中就是按这个数值升序排列的。
                                    //这样的好处是只要计算出要访问文件的CRC，就可以利用二分查找在对数时间内定位该文件的Index，进而读取数据。

    unsigned int Attrib;            //0002,0001都是文件, 区别忘了貌似是一个是压缩一个是未压缩. 而0011是已删除的文件, 0003是目录. 或许也会有0013表示已删除的目录

    unsigned int vParentCRC;        //一个CRC值，等于它的父目录的CRC。CPK文件支持子目录，当你定位好一个文件的index后，通过这个指针反复向上层遍历，
                                    //就可以取得它的完整的存储路径。在根目录下的文件的Index中此值为0。

    unsigned int Offset;            //压缩后的数据在CPK中的偏移量。

    unsigned int CompressedSize;    //压缩后数据的大小。对于目录，这个值为0。

    unsigned int OriginalSize;      //原始文件的大小，方便你解压时开缓冲区。

    unsigned int InfoRecordSize;    /*奇怪的参数。对于每一个Index所代表的文件，压缩后的数据在CPK中从index.Offset起开始存储，占用index.CompressedSize的空间，
                                      接下来就是一个大小为InfoRecordSize的奇怪记录，我只知道这个记录的一开头就是文件名，以#0结束，其他的都不清楚，有兴趣的可以研究一下。
                                      需要注意的是，只要InfoRecordSize为0，或这个Index不是目录，但CompressedSize为0，这个Index就毫无疑义，不需处理。
                                      我因为多次运行升级程序（为了调试它来研究CPK格式），文件中已有好多这样的无效Index了。*/
};

struct CPKFile {
    bool bOpened;                     //0x110
    DWORD vCRC;                      //0x114
    DWORD vParentCRC;                //0x118
    DWORD fileIndex;                //0x11C
    LPVOID lpMapFileBase;           //0x120
    void* pSrc;                     //0x124
    DWORD srcOffset;                   //0x128
    bool isCompressed;                     //0x12C
    void* pDest;                    //0x130
    DWORD originalSize;             //0x134
    DWORD fileOffset;                   //0x138
    CpkFileEntry* pRecordEntry;     //0x13C
};

//0x140
struct gbVFile {
    DWORD unknown1;                 //0x0
    DWORD unknown2;                 //0x4
    DWORD unknown3;                 //0x8
    char fileName[MAX_PATH];        //0xC
    CPKFile cpkFile;
};



struct CpkZipUnzipParam {
    int flag;
    bool bCompress;
    void* src;
    void* dest;
    DWORD srcSizeUnused;
    DWORD destSize;
    DWORD srcSize;
    DWORD destResultSize;
    bool bResult;
};

//0x80
struct CpkHeader {
    unsigned int signature; //0x0
    DWORD dwCheckFlag;      //0x4  合法的CPK文件此处值必须为1
    DWORD unknown[0x2];     //0x08
    DWORD entryCapacity;    //0x10
    DWORD unknown2[0x3];    //0x14
    unsigned int dwCount;   //0x20
    char unknown3[0x5C];
};


struct CPK {
    unsigned long dwAllocationGranularity;      //0x0
    unsigned long dwOpenMode;                   //0x4
    CpkHeader cpkHeader;                        //0x8
    CpkFileEntry entries[0x8000];               //0x88
    gbVFile* vFiles[0x8];                        //0xE0088
    bool isLoaded;                              //0xE00A8
    HANDLE fileHandle;                          //0xE0090
    HANDLE fileMappingHandle;                   //0xE0094
    char fileName[MAX_PATH];                    //0xE0098
    DWORD dwVFileOpened;                        //0xE009C
};
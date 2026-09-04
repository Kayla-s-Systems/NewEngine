use std::{
    path::Path,
    sync::mpsc::{sync_channel, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use newengine_startup_intro::{
    ResolvedStartupIntro, ResolvedStartupIntroEntry, StartupIntroNativeBackend,
    StartupIntroNativeWindow,
};
use windows::{
    core::{implement, Interface, PCWSTR},
    Win32::{
        Foundation::{COLORREF, HWND, RECT},
        Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateSolidBrush, DeleteDC,
            DeleteObject, FillRect, GdiFlush, GetDC, ReleaseDC, SelectObject, SetBrushOrgEx,
            SetStretchBltMode, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
            HALFTONE, HBITMAP, HDC, HGDIOBJ, SRCCOPY,
        },
        Media::MediaFoundation::{
            IMF2DBuffer, IMFAttributes, IMFPMediaPlayer, IMFPMediaPlayerCallback,
            IMFPMediaPlayerCallback_Impl, IMFSample, IMFSourceReader, MFCreateAttributes,
            MFCreateMediaType, MFCreateSourceReaderFromURL, MFMediaType_Video,
            MFPCreateMediaPlayer, MFShutdown, MFStartup, MFVideoFormat_RGB32, MFP_EVENT_HEADER,
            MFP_OPTION_FREE_THREADED_CALLBACK, MFSTARTUP_FULL, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE,
            MF_MT_SUBTYPE, MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED,
            MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READERF_ERROR, MF_SOURCE_READER_ALL_STREAMS,
            MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
            MF_VERSION,
        },
        System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED},
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_RETURN, VK_SPACE,
            },
            WindowsAndMessaging::{
                DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
            },
        },
    },
};

include!("presentation.rs");
include!("media.rs");
include!("gdi_frame.rs");
include!("win32_util.rs");
include!("tests.rs");

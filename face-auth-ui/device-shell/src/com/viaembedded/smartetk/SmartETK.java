package com.viaembedded.smartetk;

import android.util.Log;
import java.util.ArrayList;

/* loaded from: classes.dex */
public abstract class SmartETK {
    private static final byte CMD_INFO_VERSION = 0;
    public static final int E_CONNECTION_FAIL = -8;
    public static final int E_FAIL = -1;
    public static final int E_FUNC_NOT_SUPPORT = -6;
    public static final int E_INVALID_ARG = -4;
    public static final int E_NOT_RESPOND_YET = -9;
    public static final int E_OUT_OF_MEMORY = -5;
    public static final int E_PROTOCOL_ISSUE = -11;
    public static final int E_TIMEOUT = -10;
    public static final int E_VERSION_NOT_SUPPORT = -2;
    public static final byte FUN_AUDIO = 4;
    public static final byte FUN_CAN = 9;
    public static final byte FUN_GPIO = 1;
    public static final byte FUN_I2C = 7;
    public static final byte FUN_INFO = 0;
    public static final byte FUN_NETWORK = 6;
    public static final byte FUN_ODM = 10;
    public static final byte FUN_RTC = 3;
    public static final byte FUN_SYSTEM = 8;
    public static final byte FUN_UART = 5;
    public static final byte FUN_WDT = 2;
    public static final String LOG_TAG = "SMARTETK";
    static final int SERVICE_TIMEOUT = 3000;
    public static final int S_OK = 0;
    private static final boolean m_bDebugFlag = false;
    final byte m_byFun;
    private static final byte[] PROTOCOL_VERSION_NUMBER = {0, 0, 21};
    protected static final byte[] ETK_LIB_VERSION_NUMBER = {0, 3, 4, 0};

    public static class Timeout {
        public boolean bEnable = false;
        public int iTimeout = 0;
    }

    static void printData(int i, String str, byte[] bArr, int i2) {
    }

    abstract boolean printErrorLog(String str, int i);

    abstract int requestToService(Packet packet, byte b, byte b2, int i);

    SmartETK(byte b) {
        this.m_byFun = b;
    }

    int checkVersion(Packet packet) {
        synchronized (packet.m_byData) {
            Log.i(LOG_TAG, "SmartETK Library Protocol version is " + (PROTOCOL_VERSION_NUMBER[0] & 255) + "." + (PROTOCOL_VERSION_NUMBER[1] & 255) + "." + (PROTOCOL_VERSION_NUMBER[2] & 255) + ".");
            Log.i(LOG_TAG, "SmartETK Library version is " + (ETK_LIB_VERSION_NUMBER[0] & 255) + "." + (ETK_LIB_VERSION_NUMBER[1] & 255) + "." + (ETK_LIB_VERSION_NUMBER[2] & 255) + ".");
            packet.m_byData[0] = PROTOCOL_VERSION_NUMBER[0];
            packet.m_byData[1] = PROTOCOL_VERSION_NUMBER[1];
            packet.m_byData[2] = PROTOCOL_VERSION_NUMBER[2];
            requestToService(packet, (byte) 0, (byte) 0, 3);
            Log.i(LOG_TAG, "SmartETK service version is " + (packet.m_byData[0] & 255) + "." + (packet.m_byData[1] & 255) + "." + (packet.m_byData[2] & 255) + ". packet.m_iStatus = " + packet.m_iStatus);
            if (packet.m_iStatus == 0 && 3 > packet.m_iDataLen) {
                return printErrorLog(-11);
            }
            if (-2 == packet.m_iStatus) {
                Log.e(LOG_TAG, "SmartETK service compatible version need to be " + (ETK_LIB_VERSION_NUMBER[0] & 255) + "." + (ETK_LIB_VERSION_NUMBER[1] & 255) + "." + (ETK_LIB_VERSION_NUMBER[2] & 255) + " or above!");
            }
            if (3 <= packet.m_iDataLen) {
                Log.i(LOG_TAG, "SmartETK service version is " + (packet.m_byData[0] & 255) + "." + (packet.m_byData[1] & 255) + "." + (packet.m_byData[2] & 255) + ". packet.m_iStatus = " + packet.m_iStatus);
            }
            return packet.m_iStatus;
        }
    }

    int requestToService(ThreadSocket threadSocket, Packet packet, byte b) {
        int sendRequestToService;
        synchronized (packet.m_byData) {
            sendRequestToService = sendRequestToService(threadSocket, packet, this.m_byFun, b, null, 0, 0, null, 0, null, 0, 3000);
        }
        return sendRequestToService;
    }

    int requestToSetBoolean(ThreadSocket threadSocket, Packet packet, byte b, boolean z) {
        int sendRequestToService;
        synchronized (packet.m_byData) {
            packet.m_byData[0] = z ? (byte) 1 : (byte) 0;
            sendRequestToService = sendRequestToService(threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, 1, null, 0, null, 0, 3000);
        }
        return sendRequestToService;
    }

    int requestToSetByteBoolean(ThreadSocket threadSocket, Packet packet, byte b, int i, int i2) {
        synchronized (packet.m_byData) {
            if (i < 0 || i > 255 || i2 < 0 || i2 > 1) {
                return -4;
            }
            packet.m_byData[0] = (byte) i;
            packet.m_byData[1] = (byte) i2;
            return sendRequestToService(threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, 2, null, 0, null, 0, 3000);
        }
    }

    int requestToGetBoolean(ThreadSocket threadSocket, Packet packet, byte b, boolean[] zArr) {
        synchronized (packet.m_byData) {
            if (zArr != null) {
                if (1 == zArr.length) {
                    boolean z = true;
                    if (sendRequestToService(threadSocket, packet, this.m_byFun, b, null, 0, 0, null, 0, packet.m_byData, 0, 3000) == 0) {
                        if (1 > packet.m_iDataLen) {
                            return -11;
                        }
                        if (packet.m_byData[0] == 0) {
                            z = false;
                        }
                        zArr[0] = z;
                    }
                    return packet.m_iStatus;
                }
            }
            return -4;
        }
    }

    int requestToGetBoolean(ThreadSocket threadSocket, Packet packet, byte b, int i, boolean[] zArr) {
        synchronized (packet.m_byData) {
            if (i >= 0 && i <= 255 && zArr != null) {
                if (1 == zArr.length) {
                    packet.m_byData[0] = (byte) i;
                    if (sendRequestToService(threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, 1, null, 0, packet.m_byData, 0, 3000) == 0) {
                        if (1 > packet.m_iDataLen) {
                            return -11;
                        }
                        zArr[0] = packet.m_byData[0] != 0;
                    }
                    return packet.m_iStatus;
                }
            }
            return -4;
        }
    }

    int requestToSetByte(ThreadSocket threadSocket, Packet packet, byte b, int i) {
        synchronized (packet.m_byData) {
            if (i < 0 || i > 255) {
                return -4;
            }
            packet.m_byData[0] = (byte) i;
            return sendRequestToService(threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, 1, null, 0, null, 0, 3000);
        }
    }

    int requestToSetByte(ThreadSocket threadSocket, Packet packet, byte b, byte b2, byte b3) {
        int sendRequestToService;
        synchronized (packet.m_byData) {
            packet.m_byData[0] = b2;
            packet.m_byData[1] = b3;
            sendRequestToService = sendRequestToService(threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, 2, null, 0, null, 0, 3000);
        }
        return sendRequestToService;
    }

    int requestToSetByte(ThreadSocket threadSocket, Packet packet, byte b, int i, int i2) {
        synchronized (packet.m_byData) {
            if (i < 0 || i > 255 || i2 < 0 || i2 > 255) {
                return -4;
            }
            packet.m_byData[0] = (byte) i;
            packet.m_byData[1] = (byte) i2;
            return sendRequestToService(threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, 2, null, 0, null, 0, 3000);
        }
    }

    int requestToGetByte(ThreadSocket threadSocket, Packet packet, byte b, int i, int[] iArr) {
        synchronized (packet.m_byData) {
            if (i >= 0 && i <= 255 && iArr != null) {
                if (1 == iArr.length) {
                    packet.m_byData[0] = (byte) i;
                    if (sendRequestToService(threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, 1, null, 0, packet.m_byData, 0, 3000) == 0) {
                        if (1 > packet.m_iDataLen) {
                            return -11;
                        }
                        iArr[0] = packet.m_byData[0] & 255;
                    }
                    return packet.m_iStatus;
                }
            }
            return -4;
        }
    }

    int requestToSetInteger(ThreadSocket threadSocket, Packet packet, byte b, int i) {
        int sendRequestToService;
        synchronized (packet.m_byData) {
            intToByte(i, packet.m_byData, 0, 4);
            sendRequestToService = sendRequestToService(threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, 4, null, 0, null, 0, 3000);
        }
        return sendRequestToService;
    }

    int requestToGetInteger(ThreadSocket threadSocket, Packet packet, byte b, int[] iArr) {
        synchronized (packet.m_byData) {
            if (iArr != null) {
                if (1 == iArr.length) {
                    if (sendRequestToService(threadSocket, packet, this.m_byFun, b, null, 0, 0, null, 0, packet.m_byData, 0, 3000) == 0) {
                        if (4 > packet.m_iDataLen) {
                            return -11;
                        }
                        iArr[0] = byteToInt(packet.m_byData, 0, 4);
                    }
                    return packet.m_iStatus;
                }
            }
            return -4;
        }
    }

    int requestToGetIntegerArray(ThreadSocket threadSocket, Packet packet, byte b, ArrayList<Integer> arrayList) {
        synchronized (packet.m_byData) {
            if (arrayList == null) {
                return -4;
            }
            if (sendRequestToService(threadSocket, packet, this.m_byFun, b, null, 0, 0, null, 0, packet.m_byData, 0, 3000) == 0) {
                if (packet.m_iDataLen >= 4 && packet.m_iDataLen % 4 == 0) {
                    for (int i = 0; i < packet.m_iDataLen / 4; i++) {
                        arrayList.add(i, Integer.valueOf(byteToInt(packet.m_byData, i * 4, 4)));
                    }
                }
                return -11;
            }
            return packet.m_iStatus;
        }
    }

    int requestToGetIntegerArray(ThreadSocket threadSocket, Packet packet, byte b, String str, ArrayList<Integer> arrayList) {
        synchronized (packet.m_byData) {
            try {
                if (arrayList == null) {
                    return -4;
                }
                byte[] bytes = str != null ? str.getBytes() : null;
                if (bytes != null && bytes.length >= 0) {
                    if (bytes.length > packet.m_byData.length) {
                        return -5;
                    }
                    System.arraycopy(bytes, 0, packet.m_byData, 0, bytes.length);
                    packet.m_byData[bytes.length] = 0;
                    if (sendRequestToService(threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, bytes.length, null, 0, packet.m_byData, 0, 3000) == 0) {
                        if (packet.m_iDataLen >= 4 && packet.m_iDataLen % 4 == 0) {
                            for (int i = 0; i < packet.m_iDataLen / 4; i++) {
                                arrayList.add(i, Integer.valueOf(byteToInt(packet.m_byData, i * 4, 4)));
                            }
                        }
                        return -11;
                    }
                    return packet.m_iStatus;
                }
                return -4;
            } finally {
            }
        }
    }

    int requestToGetByteArray(ThreadSocket threadSocket, Packet packet, byte b, ArrayList<Integer> arrayList) {
        synchronized (packet.m_byData) {
            if (arrayList == null) {
                return -4;
            }
            if (sendRequestToService(threadSocket, packet, this.m_byFun, b, null, 0, 0, null, 0, packet.m_byData, 0, 3000) == 0) {
                if (packet.m_iDataLen <= 0) {
                    return -11;
                }
                for (int i = 0; i < packet.m_iDataLen; i++) {
                    arrayList.add(Integer.valueOf(packet.m_byData[i] & 255));
                }
            }
            return packet.m_iStatus;
        }
    }

    int requestToGetByteArray(ThreadSocket threadSocket, Packet packet, byte b, ArrayList<Integer> arrayList, ArrayList<Integer> arrayList2) {
        synchronized (packet.m_byData) {
            try {
                if (arrayList == null || arrayList2 == null) {
                    return -4;
                }
                try {
                    if (sendRequestToService(threadSocket, packet, this.m_byFun, b, null, 0, 0, null, 0, packet.m_byData, 0, 3000) == 0) {
                        if (packet.m_iDataLen <= 1) {
                            return -11;
                        }
                        int i = packet.m_iDataLen / 2;
                        for (int i2 = 0; i2 < i; i2++) {
                            arrayList.add(Integer.valueOf(packet.m_byData[i2] & 255));
                            arrayList2.add(Integer.valueOf(packet.m_byData[i2 + i] & 255));
                        }
                    }
                    return packet.m_iStatus;
                } catch (Throwable th) {
                    throw th;
                }
            } catch (Throwable th2) {
                throw th2;
            }
        }
    }

    int requestToGetStringArray(ThreadSocket threadSocket, Packet packet, byte b, ArrayList<String> arrayList) {
        synchronized (packet.m_byData) {
            if (arrayList == null) {
                return -4;
            }
            if (sendRequestToService(threadSocket, packet, this.m_byFun, b, null, 0, 0, null, 0, packet.m_byData, 0, 3000) == 0) {
                if (packet.m_iDataLen <= 0) {
                    return -11;
                }
                byte[] bArr = new byte[12];
                for (int i = 0; i < packet.m_iDataLen / 12; i++) {
                    System.arraycopy(packet.m_byData, i * 12, bArr, 0, 12);
                    int i2 = 0;
                    for (int i3 = 0; i3 < 12 && bArr[i3] != 0; i3++) {
                        i2++;
                    }
                    arrayList.add(new String(bArr, 0, i2));
                }
            }
            return packet.m_iStatus;
        }
    }

    int requestToGetLong(ThreadSocket threadSocket, Packet packet, byte b, long[] jArr) {
        synchronized (packet.m_byData) {
            if (jArr != null) {
                if (jArr.length == 1) {
                    if (sendRequestToService(threadSocket, packet, this.m_byFun, b, null, 0, 0, null, 0, packet.m_byData, 0, 3000) == 0) {
                        if (packet.m_iDataLen != 8) {
                            return -11;
                        }
                        jArr[0] = byteToLong(packet.m_byData, 0, 8);
                    }
                    return packet.m_iStatus;
                }
            }
            return -4;
        }
    }

    int requestToGetLong(ThreadSocket threadSocket, Packet packet, byte b, String str, long[] jArr) {
        synchronized (packet.m_byData) {
            if (jArr != null) {
                if (jArr.length == 1) {
                    byte[] bytes = str != null ? str.getBytes() : null;
                    if (bytes != null && bytes.length >= 0) {
                        if (bytes.length > packet.m_byData.length) {
                            return -5;
                        }
                        System.arraycopy(bytes, 0, packet.m_byData, 0, bytes.length);
                        packet.m_byData[bytes.length] = 0;
                        if (sendRequestToService(threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, bytes.length, null, 0, packet.m_byData, 0, 3000) == 0) {
                            if (packet.m_iDataLen != 8) {
                                return -11;
                            }
                            jArr[0] = byteToLong(packet.m_byData, 0, 8);
                        }
                        return packet.m_iStatus;
                    }
                    return -4;
                }
            }
            return -4;
        }
    }

    int requestToGetString(ThreadSocket threadSocket, Packet packet, byte b, String[] strArr) {
        synchronized (packet.m_byData) {
            if (strArr != null) {
                if (strArr.length == 1) {
                    if (sendRequestToService(threadSocket, packet, this.m_byFun, b, null, 0, 0, null, 0, packet.m_byData, 0, 3000) == 0) {
                        if (packet.m_iDataLen <= 0) {
                            return -11;
                        }
                        byte[] bArr = new byte[packet.m_iDataLen];
                        System.arraycopy(packet.m_byData, 0, bArr, 0, packet.m_iDataLen);
                        strArr[0] = new String(bArr);
                    }
                    return packet.m_iStatus;
                }
            }
            return -4;
        }
    }

    int requestToGetByte(ThreadSocket threadSocket, Packet packet, byte b, byte[] bArr) {
        synchronized (packet.m_byData) {
            if (bArr == null) {
                return -4;
            }
            if (sendRequestToService(threadSocket, packet, this.m_byFun, b, null, 0, 0, null, 0, packet.m_byData, 0, 3000) == 0) {
                if (bArr.length < packet.m_iDataLen) {
                    return -4;
                }
                System.arraycopy(packet.m_byData, 0, bArr, 0, packet.m_iDataLen);
            }
            return packet.m_iStatus;
        }
    }

    int requestToSetLong(ThreadSocket threadSocket, Packet packet, byte b, long j) {
        int sendRequestToService;
        synchronized (packet.m_byData) {
            byte[] bArr = new byte[8];
            longToByte(j, bArr, 0, 8);
            System.arraycopy(packet.m_byData, 0, bArr, 0, 8);
            sendRequestToService = sendRequestToService(threadSocket, packet, this.m_byFun, b, null, 0, 0, null, 0, packet.m_byData, 0, 3000);
        }
        return sendRequestToService;
    }

    int requestToSetString(ThreadSocket threadSocket, Packet packet, byte b, String str, int i) {
        byte[] bytes;
        int i2 = i;
        synchronized (packet.m_byData) {
            if (str != null) {
                try {
                    bytes = str.getBytes();
                } finally {
                }
            } else {
                bytes = null;
            }
            if (bytes != null && bytes.length >= 0) {
                if (bytes.length > packet.m_byData.length) {
                    return -5;
                }
                System.arraycopy(bytes, 0, packet.m_byData, 0, bytes.length);
                if (bytes.length < i2 && bytes.length < packet.m_byData.length) {
                    packet.m_byData[bytes.length] = 0;
                }
                byte b2 = this.m_byFun;
                byte[] bArr = packet.m_byData;
                if (bytes.length > i2) {
                    i2 = bytes.length;
                }
                return sendRequestToService(threadSocket, packet, b2, b, bArr, 0, i2, null, 0, null, 0, 3000);
            }
            return -4;
        }
    }

    int requestToSetString(ThreadSocket threadSocket, Packet packet, byte b, String str, String str2) {
        byte[] bytes;
        synchronized (packet.m_byData) {
            if (str != null) {
                try {
                    bytes = str.getBytes();
                } finally {
                }
            } else {
                bytes = null;
            }
            byte[] bytes2 = str2 != null ? str2.getBytes() : null;
            if (bytes != null && bytes2 != null && bytes.length >= 0 && bytes2.length >= 0) {
                int length = bytes.length + bytes2.length + 1;
                if (length > packet.m_byData.length) {
                    return -5;
                }
                System.arraycopy(bytes, 0, packet.m_byData, 0, bytes.length);
                packet.m_byData[bytes.length] = 0;
                System.arraycopy(bytes2, 0, packet.m_byData, bytes.length + 1, bytes2.length);
                return sendRequestToService(threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, length, null, 0, null, 0, 3000);
            }
            return -4;
        }
    }

    int sendRequestToService(ThreadSocket threadSocket, Packet packet, byte b, byte b2, byte[] bArr, int i, int i2, byte[] bArr2, int i3, byte[] bArr3, int i4, int i5) {
        int i6;
        if (packet == null) {
            return -1;
        }
        if (threadSocket == null) {
            packet.m_iStatus = -1;
            return -1;
        }
        synchronized (packet) {
            int sendRetuest = threadSocket.sendRetuest(packet, b, b2, bArr, i, i2, bArr2, i3, bArr3, i4);
            if (sendRetuest != 0) {
                packet.m_iStatus = sendRetuest;
            } else {
                packet.waitNotify(i5);
                threadSocket.removePacket(packet);
                printData(2, "Recv Data", bArr3, packet.m_iDataLen);
            }
            i6 = packet.m_iStatus;
        }
        return i6;
    }

    static boolean intToByte(int i, byte[] bArr, int i2, int i3) {
        if (i2 < 0 || i3 <= 0 || i3 > 4 || bArr == null || bArr.length < i2 + i3) {
            return false;
        }
        for (int i4 = 0; i4 < i3; i4++) {
            bArr[i2 + i4] = (byte) (i >> (i4 * 8));
        }
        return true;
    }

    static boolean longToByte(long j, byte[] bArr, int i, int i2) {
        if (i < 0 || i2 <= 0 || i2 > 8 || bArr == null || bArr.length < i + i2) {
            return false;
        }
        for (int i3 = 0; i3 < i2; i3++) {
            bArr[i + i3] = (byte) (j >> (i3 * 8));
        }
        return true;
    }

    static int byteToInt(byte[] bArr, int i, int i2) {
        if (i < 0 || i2 <= 0 || i2 > 4 || bArr == null || bArr.length < i + i2) {
            return 0;
        }
        int i3 = 0;
        for (int i4 = 0; i4 < i2; i4++) {
            int i5 = i4 * 8;
            i3 |= (bArr[i + i4] << i5) & (255 << i5);
        }
        return i3;
    }

    static long byteToLong(byte[] bArr, int i, int i2) {
        long j = 0;
        if (i >= 0 && i2 > 0 && i2 <= 8 && bArr != null && bArr.length >= i + i2) {
            for (int i3 = 0; i3 < i2; i3++) {
                int i4 = i3 * 8;
                j |= (bArr[i + i3] << i4) & (255 << i4);
            }
        }
        return j;
    }

    private static String getClassMethodName(int i) {
        int i2 = i + 3;
        String className = Thread.currentThread().getStackTrace().length > i2 ? Thread.currentThread().getStackTrace()[i2].getClassName() : null;
        int lastIndexOf = className == null ? -1 : className.lastIndexOf(46) + 1;
        if (lastIndexOf < 0 || lastIndexOf >= className.length()) {
            return "";
        }
        return className.substring(lastIndexOf) + "::" + Thread.currentThread().getStackTrace()[i2].getMethodName();
    }

    int printErrorLog(int i) {
        return printErrorLog(1, i);
    }

    /* JADX WARN: Can't fix incorrect switch cases order, some code will duplicate */
    /* JADX WARN: Code restructure failed: missing block: B:18:0x011f, code lost:
    
        return r3;
     */
    /*
        Code decompiled incorrectly, please refer to instructions dump.
    */
    int printErrorLog(int i, int i2) {
        if (i2 != 0) {
            String classMethodName = getClassMethodName(i + 1);
            if (!printErrorLog(classMethodName, i2)) {
                switch (i2) {
                    case -11:
                        Log.e(LOG_TAG, classMethodName + " (E_PROTOCOL_ISSUE, " + i2 + "), command data format is wrong.");
                        break;
                    case -10:
                        Log.e(LOG_TAG, classMethodName + " (E_TIMEOUT, " + i2 + "), there is no corresponding data had been received within the period.");
                        break;
                    case -9:
                        Log.e(LOG_TAG, classMethodName + " (E_NOT_RESPOND_YET, " + i2 + "), bsservice function is still running and doesn't be finished yet.");
                        break;
                    case -8:
                        Log.e(LOG_TAG, classMethodName + " (E_CONNECTION_FAIL, " + i2 + "), bsservice doesn't response the request. Please make sure bsservice is running successfully, and call resetService() or create new object again.");
                        break;
                    case -6:
                        Log.e(LOG_TAG, classMethodName + " (E_FUNC_NOT_SUPPORT, " + i2 + "), function is not supported on this platform.");
                        break;
                    case -5:
                        Log.e(LOG_TAG, classMethodName + " (E_OUT_OF_MEMORY, " + i2 + "), the length of data buffer is not enough big.");
                        break;
                    case -4:
                        Log.e(LOG_TAG, classMethodName + " (E_INVALID_ARG, " + i2 + "), arguments are invalid.");
                        break;
                    case -2:
                        Log.e(LOG_TAG, classMethodName + " (E_VERSION_NOT_SUPPORT, " + i2 + "), versions of SmartETK libraries and bsservice are not compatibility.");
                        break;
                    case -1:
                        Log.e(LOG_TAG, classMethodName + " (E_FAIL, " + i2 + "), function is failed to complete.");
                        break;
                }
            } else {
                return i2;
            }
        } else {
            return i2;
        }
        return i2;
    }

    void waitNotify(Object obj, int i) {
        if (obj != null) {
            synchronized (obj) {
                try {
                    obj.wait(i);
                } catch (InterruptedException e) {
                    e.printStackTrace();
                }
            }
        }
    }
}

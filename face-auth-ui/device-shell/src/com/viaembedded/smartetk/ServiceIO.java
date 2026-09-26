package com.viaembedded.smartetk;

import android.util.Log;
import java.util.ArrayList;

/* loaded from: classes.dex */
abstract class ServiceIO extends SmartETK {
    int m_iInitState;
    final Packet m_packet;
    public ThreadSocket m_threadSocket;

    @Override // com.viaembedded.smartetk.SmartETK
    boolean printErrorLog(String str, int i) {
        return false;
    }

    ServiceIO(byte b) {
        super(b);
        this.m_iInitState = -8;
        this.m_packet = new Packet(255);
        this.m_threadSocket = null;
        init();
        if (checkSocketIsOk() != 0) {
            Log.i(SmartETK.LOG_TAG, "Socket is not ok, call reset Service");
            resetService();
        }
    }

    private void init() {
        synchronized (ServiceIO.class) {
            if (this.m_packet == null || this.m_packet.m_byData == null || this.m_threadSocket != null) {
                return;
            }
            ThreadSocket threadSocket = new ThreadSocket();
            this.m_threadSocket = threadSocket;
            synchronized (threadSocket) {
                this.m_threadSocket.start();
                waitNotify(this.m_threadSocket, 3000);
                this.m_iInitState = this.m_threadSocket.getInitState() != 0 ? printErrorLog(1, this.m_threadSocket.getInitState()) : checkVersion(this.m_packet);
            }
        }
    }

    public void closeService() {
        synchronized (ServiceIO.class) {
            this.m_iInitState = -8;
            if (this.m_threadSocket != null) {
                try {
                    this.m_threadSocket.stopThread();
                    waitNotify(this.m_threadSocket, 3000);
                    this.m_threadSocket.join();
                } catch (InterruptedException e) {
                    e.printStackTrace();
                }
                this.m_threadSocket = null;
            }
        }
    }

    public void resetService() {
        synchronized (ServiceIO.class) {
            this.m_iInitState = -8;
            if (this.m_threadSocket != null) {
                try {
                    this.m_threadSocket.stopThread();
                    this.m_threadSocket.join();
                } catch (InterruptedException e) {
                    e.printStackTrace();
                }
                this.m_threadSocket = null;
            }
            init();
        }
    }

    private int checkSocketIsOk() {
        int i = this.m_iInitState;
        ThreadSocket threadSocket = this.m_threadSocket;
        if (threadSocket == null) {
            return i;
        }
        String[] strArr = new String[1];
        Packet packet = this.m_packet;
        return sendRequestToService(threadSocket, packet, (byte) 0, (byte) 7, null, 0, 0, null, 0, packet.m_byData, 0, 3000);
    }

    @Override // com.viaembedded.smartetk.SmartETK
    int requestToService(Packet packet, byte b, byte b2, int i) {
        return printErrorLog(1, sendRequestToService(this.m_threadSocket, packet, b, b2, packet.m_byData, 0, i, null, 0, packet.m_byData, 0, 3000));
    }

    int requestToService(Packet packet, byte b, int i) {
        return printErrorLog(1, sendRequestToService(this.m_threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, i, null, 0, packet.m_byData, 0, 3000));
    }

    int requestToService(Packet packet, byte b, int i, byte[] bArr) {
        return printErrorLog(1, sendRequestToService(this.m_threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, i, null, 0, bArr, 0, 3000));
    }

    int requestToService(Packet packet, byte b, int i, byte[] bArr, int i2) {
        return printErrorLog(1, sendRequestToService(this.m_threadSocket, packet, this.m_byFun, b, packet.m_byData, 0, i, bArr, i2, packet.m_byData, 0, 3000));
    }

    int requestToService(byte b) {
        int i = this.m_iInitState;
        if (i != 0) {
            return printErrorLog(1, i);
        }
        return printErrorLog(1, requestToService(this.m_threadSocket, this.m_packet, b));
    }

    int requestToSetBoolean(byte b, boolean z) {
        int i = this.m_iInitState;
        if (i != 0) {
            return printErrorLog(1, i);
        }
        return printErrorLog(1, requestToSetBoolean(this.m_threadSocket, this.m_packet, b, z));
    }

    int requestToSetByteBoolean(byte b, int i, int i2) {
        int i3 = this.m_iInitState;
        if (i3 != 0) {
            return printErrorLog(1, i3);
        }
        return printErrorLog(1, requestToSetByteBoolean(this.m_threadSocket, this.m_packet, b, i, i2));
    }

    int requestToGetBoolean(byte b, boolean[] zArr) {
        int i = this.m_iInitState;
        if (i != 0) {
            return printErrorLog(1, i);
        }
        return printErrorLog(1, requestToGetBoolean(this.m_threadSocket, this.m_packet, b, zArr));
    }

    int requestToGetBoolean(byte b, int i, boolean[] zArr) {
        int i2 = this.m_iInitState;
        if (i2 != 0) {
            return printErrorLog(1, i2);
        }
        return printErrorLog(1, requestToGetBoolean(this.m_threadSocket, this.m_packet, b, i, zArr));
    }

    int requestToSetByte(byte b, int i, int i2) {
        int i3 = this.m_iInitState;
        if (i3 != 0) {
            return printErrorLog(1, i3);
        }
        return printErrorLog(1, requestToSetByte(this.m_threadSocket, this.m_packet, b, i, i2));
    }

    int requestToGetByte(byte b, int i, int[] iArr) {
        int i2 = this.m_iInitState;
        if (i2 != 0) {
            return printErrorLog(1, i2);
        }
        return printErrorLog(1, requestToGetByte(this.m_threadSocket, this.m_packet, b, i, iArr));
    }

    int requestToSetInteger(byte b, int i) {
        int i2 = this.m_iInitState;
        if (i2 != 0) {
            return printErrorLog(1, i2);
        }
        return printErrorLog(1, requestToSetInteger(this.m_threadSocket, this.m_packet, b, i));
    }

    int requestToSetString(byte b, String str, int i) {
        int i2 = this.m_iInitState;
        if (i2 != 0) {
            return printErrorLog(1, i2);
        }
        return printErrorLog(1, requestToSetString(this.m_threadSocket, this.m_packet, b, str, i));
    }

    int requestToSetString(byte b, String str, String str2) {
        int i = this.m_iInitState;
        if (i != 0) {
            return printErrorLog(1, i);
        }
        return printErrorLog(1, requestToSetString(this.m_threadSocket, this.m_packet, b, str, str2));
    }

    int requestToGetInteger(byte b, int[] iArr) {
        int i = this.m_iInitState;
        if (i != 0) {
            return printErrorLog(1, i);
        }
        return printErrorLog(1, requestToGetInteger(this.m_threadSocket, this.m_packet, b, iArr));
    }

    int requestToGetIntegerArray(byte b, ArrayList<Integer> arrayList) {
        int i = this.m_iInitState;
        if (i != 0) {
            return printErrorLog(1, i);
        }
        return printErrorLog(1, requestToGetIntegerArray(this.m_threadSocket, this.m_packet, b, arrayList));
    }

    int requestToGetByteArray(byte b, ArrayList<Integer> arrayList) {
        int i = this.m_iInitState;
        if (i != 0) {
            return printErrorLog(1, i);
        }
        return printErrorLog(1, requestToGetByteArray(this.m_threadSocket, this.m_packet, b, arrayList));
    }

    int requestToGetByteArray(byte b, ArrayList<Integer> arrayList, ArrayList<Integer> arrayList2) {
        int i = this.m_iInitState;
        if (i != 0) {
            return printErrorLog(1, i);
        }
        return printErrorLog(1, requestToGetByteArray(this.m_threadSocket, this.m_packet, b, arrayList, arrayList2));
    }

    int requestToGetLong(byte b, long[] jArr) {
        int i = this.m_iInitState;
        if (i != 0) {
            return printErrorLog(1, i);
        }
        return printErrorLog(1, requestToGetLong(this.m_threadSocket, this.m_packet, b, jArr));
    }

    int requestToGetString(byte b, String[] strArr) {
        int i = this.m_iInitState;
        if (i != 0) {
            return printErrorLog(1, i);
        }
        return printErrorLog(1, requestToGetString(this.m_threadSocket, this.m_packet, b, strArr));
    }

    int requestToGetByte(byte b, byte[] bArr) {
        int i = this.m_iInitState;
        if (i != 0) {
            return printErrorLog(1, i);
        }
        return printErrorLog(1, requestToGetByte(this.m_threadSocket, this.m_packet, b, bArr));
    }
}

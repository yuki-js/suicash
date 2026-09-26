package com.viaembedded.smartetk;

import android.util.Log;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.Socket;
import java.net.SocketException;
import java.net.SocketTimeoutException;
import java.net.UnknownHostException;

/* loaded from: classes.dex */
final class ThreadSocket extends Thread {
    static final int HEADER_COMMAND = 2;
    static final int HEADER_DATA_LENGTH = 4;
    static final int HEADER_FCUNTION = 1;
    static final int HEADER_PREFIX = 0;
    static final int HEADER_REQUEST_POSTFIX = 5;
    static final int HEADER_RETURN_POSTFIX = 6;
    static final int HEADER_RETURN_STATUS = 5;
    static final int HEADER_SERIAL_NUMBER = 3;
    static final byte POSTFIX_EXTENSION_DATASIZE = -4;
    static final byte POSTFIX_GENERAL_DATASIZE = -3;
    private static final byte PREFIX_REQUEST_FUNCTION = -1;
    private static final byte PREFIX_RETURN_STATUS = -2;
    static final int REQUEST_HEADER_LENGTH = 6;
    static final int RETURN_HEADER_LENGTH = 7;
    private static final String SMARTETK_SERVICEADDRESS = "127.0.0.1";
    private static final int SMARTETK_SERVICEPORT = 49582;
    private static final int SOCKET_BUFFER_SIZE = 1408;
    private final PacketList m_packetList = new PacketList();
    private final byte[] m_bySendPacket = new byte[SOCKET_BUFFER_SIZE];
    private byte m_bySerialNumber = 0;
    private int m_iInitState = -8;
    private Socket m_socket = null;
    private InputStream m_iptSocket = null;
    private OutputStream m_optSocket = null;
    private boolean m_bThreadFlag = false;

    ThreadSocket() {
    }

    @Override // java.lang.Thread, java.lang.Runnable
    public void run() {
        if (initConnection()) {
            readThread();
        }
        release();
    }

    void removePacket(Packet packet) {
        PacketList packetList = this.m_packetList;
        if (packetList != null) {
            packetList.remove(packet);
        }
    }

    synchronized int getInitState() {
        return this.m_iInitState;
    }

    synchronized int sendRetuest(Packet packet, byte b, byte b2, byte[] bArr, int i, int i2, byte[] bArr2, int i3, byte[] bArr3, int i4) {
        if (this.m_optSocket == null || this.m_bySendPacket == null || this.m_packetList == null) {
            return -8;
        }
        int i5 = i2 + 6 + i3 + (i2 + i3 < 256 ? 0 : 4);
        if (i >= 0 && i2 >= 0 && ((bArr != null || i2 == 0) && ((bArr == null || i + i2 <= bArr.length) && ((bArr2 != null || i3 == 0) && ((bArr2 == null || i3 <= bArr2.length) && i5 <= this.m_bySendPacket.length))))) {
            this.m_bySendPacket[0] = -1;
            this.m_bySendPacket[1] = b;
            this.m_bySendPacket[2] = b2;
            byte[] bArr4 = this.m_bySendPacket;
            byte b3 = (byte) (this.m_bySerialNumber + 1);
            this.m_bySerialNumber = b3;
            bArr4[3] = b3;
            if (i2 < 256) {
                this.m_bySendPacket[4] = (byte) i2;
                this.m_bySendPacket[5] = POSTFIX_GENERAL_DATASIZE;
            } else {
                this.m_bySendPacket[4] = 4;
                this.m_bySendPacket[5] = POSTFIX_EXTENSION_DATASIZE;
                SmartETK.intToByte(i2, this.m_bySendPacket, 6, 4);
            }
            if (i2 > 0) {
                System.arraycopy(bArr, i, this.m_bySendPacket, (i5 - i3) - i2, i2);
            }
            if (i3 > 0) {
                System.arraycopy(bArr2, 0, this.m_bySendPacket, i5 - i3, i3);
            }
            SmartETK.printData(3, "Send Data", this.m_bySendPacket, i5);
            try {
                packet.setPacket(b, b2, this.m_bySendPacket[3], bArr3, i4);
                this.m_packetList.push(packet);
                this.m_optSocket.write(this.m_bySendPacket, 0, i5);
                this.m_optSocket.flush();
                return 0;
            } catch (IOException e) {
                Log.e(SmartETK.LOG_TAG, "socket output stream exception!! (" + e.getMessage() + ")");
                packet.disableWaitFlag();
                this.m_packetList.remove(packet);
                return -8;
            }
        }
        return -1;
    }

    private synchronized boolean initConnection() {
        if (this.m_packetList == null || this.m_iInitState == 0) {
            return false;
        }
        try {
            Socket socket = new Socket(SMARTETK_SERVICEADDRESS, SMARTETK_SERVICEPORT);
            this.m_socket = socket;
            try {
                this.m_optSocket = socket.getOutputStream();
                try {
                    this.m_iptSocket = this.m_socket.getInputStream();
                    this.m_iInitState = 0;
                    this.m_bThreadFlag = true;
                    notify();
                    return true;
                } catch (IOException e) {
                    Log.e(SmartETK.LOG_TAG, "Establish socket input stream failed! (" + e.getMessage() + ")");
                    return false;
                }
            } catch (IOException e2) {
                Log.e(SmartETK.LOG_TAG, "Establish socket output stream failed! (" + e2.getMessage() + ")");
                return false;
            }
        } catch (UnknownHostException e3) {
            e3.printStackTrace();
            return false;
        } catch (IOException e4) {
            Log.e(SmartETK.LOG_TAG, "Establish socket failed!!");
            Log.e(SmartETK.LOG_TAG, e4.getMessage());
            return false;
        }
    }

    synchronized void stopThread() {
        this.m_iInitState = -8;
        this.m_bThreadFlag = false;
        if (this.m_socket != null) {
            try {
                this.m_socket.close();
            } catch (IOException e) {
                e.printStackTrace();
            }
        }
    }

    private synchronized void release() {
        this.m_iInitState = -8;
        this.m_bThreadFlag = false;
        cleanConnection();
        if (this.m_packetList != null) {
            this.m_packetList.notifyAll(-8);
        }
        notify();
    }

    private synchronized void cleanConnection() {
        if (this.m_optSocket != null) {
            try {
                this.m_optSocket.flush();
                this.m_optSocket.close();
            } catch (IOException e) {
                e.printStackTrace();
            }
            this.m_optSocket = null;
        }
        if (this.m_iptSocket != null) {
            try {
                this.m_iptSocket.close();
            } catch (IOException e2) {
                e2.printStackTrace();
            }
            this.m_iptSocket = null;
        }
        if (this.m_socket != null) {
            try {
                this.m_socket.close();
            } catch (IOException e3) {
                e3.printStackTrace();
            }
            this.m_socket = null;
        }
    }

    private int parsePacket(byte[] bArr, int i) {
        int i2;
        int i3;
        if (bArr == null || i <= 0 || i > bArr.length) {
            return 0;
        }
        int i4 = i;
        while (i4 > 0) {
            int i5 = i - i4;
            while (i5 < i && -2 != bArr[i5]) {
                i5++;
            }
            int i6 = i - i5;
            if (i6 >= 7) {
                int i7 = i5 + 6;
                if (-3 == bArr[i7] || -4 == bArr[i7]) {
                    int i8 = i6 - 7;
                    int i9 = i5 + 4;
                    if (i8 >= (bArr[i9] & 255)) {
                        if (-4 == bArr[i7]) {
                            i2 = (bArr[i9] & 255) <= 4 ? SmartETK.byteToInt(bArr, i5 + 7, bArr[i9] & 255) : 0;
                            if ((bArr[i9] & 255) > 4 || i2 < 0 || i2 > (bArr.length - 7) - (bArr[i9] & 255)) {
                                this.m_packetList.notifyError(bArr, i5, -11);
                                i3 = (bArr[i9] & 255) + 7;
                                i4 = i6 - i3;
                            } else if (i8 - (bArr[i9] & 255) < i2) {
                            }
                        } else {
                            i2 = 0;
                        }
                        this.m_packetList.notifyPacket(bArr, i5);
                        i3 = (bArr[i9] & 255) + 7 + i2;
                        i4 = i6 - i3;
                    }
                } else {
                    i4 = i6 - 1;
                }
            }
            i4 = i6;
            break;
        }
        System.arraycopy(bArr, i - i4, bArr, 0, i4);
        return i4;
    }

    private void readThread() {
        int i;
        byte[] bArr = new byte[SOCKET_BUFFER_SIZE];
        int i2 = 0;
        while (this.m_bThreadFlag) {
            try {
                i = this.m_iptSocket.read(bArr, i2, 1408 - i2);
            } catch (SocketTimeoutException unused) {
                Log.e(SmartETK.LOG_TAG, "socket read: Timeout! Release buffers...");
                if (i2 >= 7) {
                    this.m_packetList.notifyError(bArr, 0, -10);
                }
                i2 = 0;
                i = 0;
            } catch (IOException e) {
                Log.e(SmartETK.LOG_TAG, "socket input stream exception!! (" + e.getMessage() + ")");
                return;
            }
            if (i < 0) {
                return;
            }
            i2 = parsePacket(bArr, i2 + i);
            if (i2 > 0) {
                try {
                    this.m_socket.setSoTimeout(3000);
                } catch (SocketException unused2) {
                    Log.e(SmartETK.LOG_TAG, "socket closed!!");
                    return;
                }
            } else {
                try {
                    this.m_socket.setSoTimeout(0);
                } catch (SocketException unused3) {
                    Log.e(SmartETK.LOG_TAG, "socket closed!!");
                    return;
                }
            }
        }
    }
}

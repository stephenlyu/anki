import aqt.forms.register
from aqt.qt import *
import aqt
import aqt.forms
import re
import requests
import json

def validate_email(email):
    # 定义电子邮件正则表达式
    pattern = r'^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$'
    if re.match(pattern, email):
        return True
    return False

class RegisterDialog(QDialog, aqt.forms.register.Ui_registerDialog):
    def __init__(self, mw):
        super().__init__()
        self.mw = mw
        self.setupUi(self) 
        self.btnRegister.clicked.connect(self.on_register)
        self.btnCancel.clicked.connect(self.reject)

    def on_register(self):
        email = self.leEmail.text().strip()
        if email == '':
            QMessageBox.warning(self, '警告', '邮箱地址为空')
            return
        if not validate_email(email):
            QMessageBox.warning(self, '警告', '邮箱地址不合法')
            return
        name = self.leName.text().strip()
        password = self.lePassword.text().strip()
        if password == '':
            QMessageBox.warning(self, '警告', '密码为空')
            return
        password_confirm = self.lePasswordConfirm.text().strip()
        if password != password_confirm:
            QMessageBox.warning(self, '警告', '两次输入的密码不一致!')
            return
        mw = self.mw

        mw.taskman.with_progress(
                lambda: self.do_sign_up(email, name, password),
                self.on_sign_up_done,
                parent=mw,
            )
        
    def do_sign_up(self, email, name, password):
        endpoint = self.mw.pm.sync_endpoint()
        url = f"{endpoint}register"

        # 注册数据
        data = {
            "email": email,
            "name": name,
            "password": password
        }

        # 设置请求头，指定内容类型为 JSON
        headers = {
            "Content-Type": "application/json"
        }

        try:
            # 发送 POST 请求
            response = requests.post(url, data=json.dumps(data), headers=headers)
            return (None, response.json())
        except requests.exceptions.RequestException as e:
            return (e, None)

    def on_sign_up_done(self, fut):
        try:
            err, result = fut.result()
            if err:
                QMessageBox.warning(self, '警告', '请求错误!')
            else:
                if result['status'] == 200:
                    QMessageBox.information(self, '信息', '注册成功!')
                    self.accept()
                else:
                    if result['message'] == 'account_exists':
                        QMessageBox.warning(self, '警告', '邮箱已被注册!')
                    else:
                        QMessageBox.warning(self, '警告', '服务器错误!')
        except: 
            QMessageBox.warning(self, '警告', '请求错误!')
